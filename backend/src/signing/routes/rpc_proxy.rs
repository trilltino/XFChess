//! Read/submit-only Solana RPC proxy for the distributed game client.
//!
//! The packaged client can't embed the paid Triton RPC URL directly — it
//! carries a secret x-token in the path (see `signing::solana::rpc::redact_url`)
//! — without leaking that token to every player who downloads the game. So the
//! client instead points `SOLANA_RPC_URL` at this route, and the backend
//! forwards to the real endpoint server-side, where the token stays.
//!
//! This is a public, unauthenticated route baked into every binary, so it is
//! restricted to a small allow-list of JSON-RPC methods the client actually
//! needs (see `ALLOWED_METHODS`) plus a per-IP rate limit — otherwise anyone
//! who extracts the URL from the binary could run arbitrary (and possibly
//! expensive) RPC calls against our paid provider account.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::signing::AppState;

/// JSON-RPC methods the client is allowed to call through this proxy.
/// Deliberately excludes anything that could run up costs on our paid RPC
/// account (`getProgramAccounts`, `getBlock`/`getBlocks`, …) or that has no
/// business being called by a player (`requestAirdrop`).
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
/// Generous enough for a client polling account/balance state every 1-3s
/// across a couple of concurrent games; tight enough to bound abuse of the
/// upstream paid RPC from a single source.
const RATE_LIMIT_MAX_PER_WINDOW: usize = 180;

fn rate_tracker() -> &'static Mutex<HashMap<String, Vec<Instant>>> {
    static TRACKER: OnceLock<Mutex<HashMap<String, Vec<Instant>>>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(HashMap::new()))
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

fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}

/// Returns true if `ip` has exceeded its request budget for the window.
async fn rate_limited(ip: &str) -> bool {
    let mut tracker = rate_tracker().lock().await;
    let now = Instant::now();
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

/// A JSON-RPC request body is either a single object or a batch array —
/// every method named in it must be on the allow-list.
fn method_allowed(body: &Value) -> bool {
    match body.as_array() {
        Some(items) => !items.is_empty() && items.iter().all(is_allowed_method),
        None => is_allowed_method(body),
    }
}

/// Shared forwarding logic for both clusters — validates, rate-limits, then
/// relays the raw JSON-RPC body to `upstream_url` and mirrors its response.
async fn forward(headers: &HeaderMap, body: &Value, upstream_url: &str) -> Response {
    let ip = client_ip(headers);
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

/// POST /api/rpc — forwards an allow-listed JSON-RPC call to the real Solana
/// devnet RPC endpoint (game accounts, moves, wagers). Exists so the
/// distributed client gets fast `confirmed` reads/sends without embedding the
/// paid provider's secret token in every binary — see module docs.
async fn proxy_rpc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    forward(&headers, &body, &state.solana_rpc_url).await
}

/// POST /api/rpc/mainnet — same as `/api/rpc` but forwards to a dedicated
/// mainnet RPC endpoint (`SOLANA_MAINNET_RPC_URL`), for reads unrelated to
/// in-game devnet state — e.g. the wallet HUD's real SOL balance. Falls back
/// to the free public mainnet RPC if no dedicated endpoint is configured
/// (same public endpoint the client used to hit directly and unproxied).
async fn proxy_rpc_mainnet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let upstream_url = state
        .config
        .solana_mainnet_rpc_url
        .clone()
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
    forward(&headers, &body, &upstream_url).await
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
