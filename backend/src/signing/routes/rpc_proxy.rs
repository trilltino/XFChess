use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::signing::AppState;

const ALLOWED_METHODS: &[&str] = &[
    "getLatestBlockhash",
    "isBlockhashValid",
    "getAccountInfo",
    "getMultipleAccounts",
    "getBalance",
    "sendTransaction",
    "getSignatureStatuses",
    "simulateTransaction",
    "getVersion",
    "getHealth",
];

const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX_PER_WINDOW: usize = 180;

const RATE_LIMIT_MAX_TRACKED_SOURCES: usize = 50_000;

fn rate_tracker() -> &'static Mutex<HashMap<String, Vec<Instant>>> {
    static TRACKER: OnceLock<Mutex<HashMap<String, Vec<Instant>>>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trusted_proxies() -> &'static Vec<String> {
    static TRUSTED: OnceLock<Vec<String>> = OnceLock::new();
    TRUSTED.get_or_init(|| {
        std::env::var("TRUSTED_PROXY_IPS")
            .unwrap_or_else(|_| "127.0.0.1,::1".to_string())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client")
    })
}

struct PeerAddr(Option<SocketAddr>);

impl<S> axum::extract::FromRequestParts<S> for PeerAddr
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(PeerAddr(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0),
        ))
    }
}

fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    let peer_ip = peer
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown-peer".to_string());

    if trusted_proxies().iter().any(|t| *t == peer_ip) {
        if let Some(forwarded) = headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            // `x-forwarded-for` is a list; the left-most entry is the origin.
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            return forwarded.to_string();
        }
    }

    peer_ip
}

async fn rate_limited(ip: &str) -> bool {
    let mut tracker = rate_tracker().lock().await;
    let now = Instant::now();

    tracker.retain(|key, hits| {
        if key == ip {
            return true; // handled below, don't drop the bucket we're about to use
        }
        hits.retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);
        !hits.is_empty()
    });

    if !tracker.contains_key(ip) && tracker.len() >= RATE_LIMIT_MAX_TRACKED_SOURCES {
        tracing::warn!(
            "[rpc_proxy] tracking {} distinct sources — refusing new ones until the window clears",
            tracker.len()
        );
        return true;
    }

    let hits = tracker.entry(ip.to_string()).or_default();
    hits.retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);
    if hits.len() >= RATE_LIMIT_MAX_PER_WINDOW {
        return true;
    }
    hits.push(now);
    false
}

fn is_allowed_method(v: &Value) -> bool {
    v.get("method")
        .and_then(Value::as_str)
        .map(|m| ALLOWED_METHODS.contains(&m))
        .unwrap_or(false)
}

fn method_allowed(body: &Value) -> bool {
    match body.as_array() {
        Some(items) => !items.is_empty() && items.iter().all(is_allowed_method),
        None => is_allowed_method(body),
    }
}

async fn forward(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    body: &Value,
    upstream_url: &str,
) -> Response {
    let ip = client_ip(headers, peer);
    if rate_limited(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response();
    }

    if !method_allowed(body) {
        return (StatusCode::FORBIDDEN, "method not allowed").into_response();
    }

    let upstream = http_client().post(upstream_url).json(body).send().await;

    match upstream {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.bytes().await {
                Ok(bytes) => (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    bytes,
                )
                    .into_response(),
                Err(_) => (StatusCode::BAD_GATEWAY, "upstream read error").into_response(),
            }
        }
        Err(e) => {
            tracing::warn!("[rpc_proxy] upstream request failed: {e}");
            (StatusCode::BAD_GATEWAY, "upstream unreachable").into_response()
        }
    }
}

async fn proxy_rpc(
    State(state): State<AppState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    forward(&headers, peer, &body, &state.solana_rpc_url).await
}

async fn proxy_rpc_mainnet(
    State(state): State<AppState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let upstream_url = state
        .config
        .solana_mainnet_rpc_url
        .clone()
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    forward(&headers, peer, &body, &upstream_url).await
}

pub fn rpc_proxy_routes() -> Router<AppState> {
    Router::new()
        .route("/rpc", post(proxy_rpc))
        .route("/rpc/mainnet", post(proxy_rpc_mainnet))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_known_single_method() {
        let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash", "params": []});
        assert!(method_allowed(&body));
    }

    #[test]
    fn rejects_unknown_single_method() {
        let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "getProgramAccounts", "params": []});
        assert!(!method_allowed(&body));
    }

    #[test]
    fn rejects_batch_with_any_disallowed_method() {
        let body = serde_json::json!([
            {"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash", "params": []},
            {"jsonrpc": "2.0", "id": 2, "method": "getProgramAccounts", "params": []}
        ]);
        assert!(!method_allowed(&body));
    }

    #[test]
    fn allows_batch_of_known_methods() {
        let body = serde_json::json!([
            {"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash", "params": []},
            {"jsonrpc": "2.0", "id": 2, "method": "getBalance", "params": []}
        ]);
        assert!(method_allowed(&body));
    }

    #[test]
    fn rejects_empty_batch() {
        let body = serde_json::json!([]);
        assert!(!method_allowed(&body));
    }

    #[tokio::test]
    async fn rate_limit_trips_after_budget_exhausted() {
        let ip = "203.0.113.7-rate-limit-test";
        for _ in 0..RATE_LIMIT_MAX_PER_WINDOW {
            assert!(!rate_limited(ip).await);
        }
        assert!(rate_limited(ip).await);
    }
}
