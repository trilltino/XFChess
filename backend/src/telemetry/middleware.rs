use axum::{
    body::Body,
    extract::{MatchedPath, Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

use super::logging::RequestContext;
use crate::signing::AppState;

pub const HTTP_STATUS_SERVER_ERROR: u16 = 500;

pub const HTTP_STATUS_CLIENT_ERROR: u16 = 400;

/// The metrics label for a request: its method and route template. Unknown
/// methods and unrouted paths share one label each, so requests for arbitrary
/// URLs cannot grow the metric set without bound.
fn endpoint_label(method: &Method, route: Option<&MatchedPath>) -> String {
    const METHODS: [Method; 7] = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::HEAD,
        Method::OPTIONS,
    ];
    let method = if METHODS.contains(method) {
        method.as_str()
    } else {
        "OTHER"
    };
    let route = route.map_or("unmatched", MatchedPath::as_str);
    format!("{method} {route}")
}

pub async fn telemetry_middleware(
    State(app_state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let endpoint = endpoint_label(&method, request.extensions().get::<MatchedPath>());
    let benchmark_run = request
        .headers()
        .get("x-benchmark-run")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 128)
        .unwrap_or("")
        .to_owned();
    let context = RequestContext::new(&endpoint);

    // Log routine polls at debug while retaining metrics and error reporting.
    let is_routine_poll = matches!(path.as_str(), "/region" | "/presence");

    if is_routine_poll {
        tracing::debug!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            benchmark_run = %benchmark_run,
            "request_started"
        );
    } else {
        tracing::info!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            benchmark_run = %benchmark_run,
            "request_started"
        );
    }

    let response = next.run(request).await;

    let duration = start.elapsed();
    let duration_ms = duration.as_millis() as f64;
    let status = response.status().as_u16();

    // Record metrics (Metrics uses interior mutability — no lock needed here).
    app_state
        .metrics
        .record_http_request(&endpoint, status, duration_ms);

    // tracing levels must be constants, hence one branch per level.
    if status >= HTTP_STATUS_SERVER_ERROR {
        tracing::error!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    } else if status >= HTTP_STATUS_CLIENT_ERROR {
        tracing::warn!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    } else if is_routine_poll {
        tracing::debug!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    } else {
        tracing::info!(
            request_id = %context.request_id,
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    #[tokio::test]
    async fn test_request_context_creation() {
        let ctx = RequestContext::new("GET /api/test");
        assert_eq!(ctx.endpoint, "GET /api/test");
        assert!(ctx.elapsed_ms() < 100); // Should be very fast
    }

    #[test]
    fn unrouted_paths_share_one_metrics_label() {
        assert_eq!(endpoint_label(&Method::GET, None), "GET unmatched");
    }

    #[test]
    fn unknown_methods_share_one_metrics_label() -> anyhow::Result<()> {
        let custom = Method::from_bytes(b"PROBE123").context("test setup failed")?;
        assert_eq!(endpoint_label(&custom, None), "OTHER unmatched");
        Ok(())
    }
}
