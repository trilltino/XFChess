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

fn http_client() -> Result<&'static reqwest::Client, StatusCode> {
    static CLIENT: OnceLock<Result<reqwest::Client, reqwest::Error>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
        })
        .as_ref()
        .map_err(|error| {
            tracing::error!("RPC HTTP client unavailable: {error}");
            StatusCode::SERVICE_UNAVAILABLE
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

    if trusted_proxies().contains(&peer_ip) {
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

    static LAST_SWEEP: OnceLock<std::sync::Mutex<Instant>> = OnceLock::new();
    let sweep = LAST_SWEEP.get_or_init(|| std::sync::Mutex::new(now));
    // Each source prunes its own bucket. Global cleanup runs at most once per second.
    if let Ok(mut last) = sweep.lock() {
        if now.duration_since(*last) >= Duration::from_secs(1) {
            tracker.retain(|_, hits| {
                hits.last()
                    .is_some_and(|hit| now.duration_since(*hit) < RATE_LIMIT_WINDOW)
            });
            *last = now;
        }
    }

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

// Recovery needs Game discovery even after the local ledger is lost. Keep
// general program scans blocked: allow only Game + nonzero player filters for
// this deployment's program or delegated Game accounts.
fn wallet_game_scan_allowed(body: &Value, program: &solana_sdk::pubkey::Pubkey) -> bool {
    use sha2::{Digest, Sha256};
    use solana_client::{rpc_config::RpcProgramAccountsConfig, rpc_filter::RpcFilterType};
    if body["method"].as_str() != Some("getProgramAccounts") {
        return false;
    }
    let Some(params) = body["params"].as_array() else {
        return false;
    };
    if params.len() != 2 {
        return false;
    }
    let owner = params[0].as_str().unwrap_or_default();
    if owner != program.to_string() && owner != crate::signing::solana::DELEGATION_PROGRAM_ID {
        return false;
    }
    let Ok(config) = serde_json::from_value::<RpcProgramAccountsConfig>(params[1].clone()) else {
        return false;
    };
    let Some(filters) = config.filters else {
        return false;
    };
    if filters.len() != 2 {
        return false;
    }
    let mut discriminator = false;
    let mut wallet = false;
    for filter in filters {
        let RpcFilterType::Memcmp(memcmp) = filter else {
            return false;
        };
        let Some(bytes) = memcmp.bytes() else {
            return false;
        };
        if memcmp.offset() == 0 && bytes.as_slice() == &Sha256::digest(b"account:Game")[..8] {
            discriminator = true;
        } else if matches!(memcmp.offset(), 16 | 48)
            && bytes.len() == 32
            && bytes.iter().any(|b| *b != 0)
        {
            wallet = true;
        } else {
            return false;
        }
    }
    discriminator && wallet
}

fn request_allowed(body: &Value, program: Option<&solana_sdk::pubkey::Pubkey>) -> bool {
    if method_allowed(body) {
        return true;
    }
    let allowed = |item: &Value| {
        is_allowed_method(item)
            || program.is_some_and(|program| wallet_game_scan_allowed(item, program))
    };
    match body.as_array() {
        Some(items) => !items.is_empty() && items.iter().all(allowed),
        None => allowed(body),
    }
}

async fn forward(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    body: &Value,
    upstream_url: &str,
    program: Option<&solana_sdk::pubkey::Pubkey>,
) -> Response {
    let ip = client_ip(headers, peer);
    if rate_limited(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response();
    }

    if !request_allowed(body, program) {
        return (StatusCode::FORBIDDEN, "method not allowed").into_response();
    }

    let client = match http_client() {
        Ok(client) => client,
        Err(status) => return status.into_response(),
    };
    let upstream = client.post(upstream_url).json(body).send().await;

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
    forward(
        &headers,
        peer,
        &body,
        &state.solana_rpc_url,
        Some(&state.program_id),
    )
    .await
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
    forward(&headers, peer, &body, &upstream_url, None).await
}

pub fn rpc_proxy_routes() -> Router<AppState> {
    Router::new()
        .route("/rpc", post(proxy_rpc))
        .route("/rpc/mainnet", post(proxy_rpc_mainnet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

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
    fn recovery_scan_requires_the_game_program_discriminator_and_a_player_filter(
    ) -> anyhow::Result<()> {
        use sha2::{Digest, Sha256};
        use solana_client::{
            rpc_config::RpcProgramAccountsConfig,
            rpc_filter::{Memcmp, RpcFilterType},
        };
        use solana_sdk::pubkey::Pubkey;
        let program = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                    0,
                    Sha256::digest(b"account:Game")[..8].to_vec(),
                )),
                RpcFilterType::Memcmp(Memcmp::new_raw_bytes(16, wallet.to_bytes().to_vec())),
            ]),
            ..Default::default()
        };
        let mut body = serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"getProgramAccounts", "params":[program.to_string(), config]});
        assert!(request_allowed(&body, Some(&program)));
        assert!(!request_allowed(&body, None));
        body["params"][0] = serde_json::json!(crate::signing::solana::DELEGATION_PROGRAM_ID);
        assert!(request_allowed(&body, Some(&program)));
        body["params"][0] = serde_json::json!(Pubkey::new_unique().to_string());
        assert!(!request_allowed(&body, Some(&program)));
        body["params"][0] = serde_json::json!(program.to_string());
        body["params"][1]["filters"]
            .as_array_mut()
            .context("test setup failed")?
            .pop();
        assert!(!request_allowed(&body, Some(&program)));
        Ok(())
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
