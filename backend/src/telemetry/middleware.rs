//! Axum middleware for request telemetry
//!
//! Injects request tracing context and records metrics for all HTTP requests.

use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::time::Instant;

use super::logging::RequestContext;
use super::metrics::Metrics;

/// HTTP status code threshold for server errors.
pub const HTTP_STATUS_SERVER_ERROR: u16 = 500;

/// HTTP status code threshold for client errors.
pub const HTTP_STATUS_CLIENT_ERROR: u16 = 400;

/// Middleware that adds telemetry to all requests
pub async fn telemetry_middleware(
    State(metrics): State<std::sync::Arc<tokio::sync::RwLock<Metrics>>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let endpoint = format!("{} {}", request.method(), request.uri().path());
    let context = RequestContext::new(&endpoint);

    // Log request start
    tracing::info!(
        request_id = %context.request_id,
        method = %request.method(),
        path = %request.uri().path(),
        "request_started"
    );

    // Process request
    let response = next.run(request).await;

    // Calculate duration
    let duration = start.elapsed();
    let duration_ms = duration.as_millis() as f64;
    let status = response.status().as_u16();

    // Record metrics
    {
        let mut metrics_guard = metrics.write().await;
        metrics_guard.record_http_request(&endpoint, status, duration_ms);
    }

    // Log request completion
    if status as u16 >= HTTP_STATUS_SERVER_ERROR {
        tracing::error!(
            request_id = %context.request_id,
            method = %context.endpoint.split_whitespace().next().unwrap_or("UNKNOWN"),
            path = %context.endpoint.split_whitespace().nth(1).unwrap_or("/"),
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    } else if status >= HTTP_STATUS_CLIENT_ERROR {
        tracing::warn!(
            request_id = %context.request_id,
            method = %context.endpoint.split_whitespace().next().unwrap_or("UNKNOWN"),
            path = %context.endpoint.split_whitespace().nth(1).unwrap_or("/"),
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    } else {
        tracing::info!(
            request_id = %context.request_id,
            method = %context.endpoint.split_whitespace().next().unwrap_or("UNKNOWN"),
            path = %context.endpoint.split_whitespace().nth(1).unwrap_or("/"),
            status = status,
            duration_ms = duration_ms,
            "request_completed"
        );
    }

    response
}

/// Middleware that extracts wallet/game info from JWT/auth headers
pub async fn extract_context_middleware(request: Request<Body>, next: Next) -> Response {
    // Try to extract wallet pubkey from Authorization header or JWT
    // This is a placeholder - actual implementation depends on your auth structure

    // For now, just pass through
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_context_creation() {
        let ctx = RequestContext::new("GET /api/test");
        assert_eq!(ctx.endpoint, "GET /api/test");
        assert!(ctx.elapsed_ms() < 100); // Should be very fast
    }
}
