//! Shared HTTP client configuration for VPS calls.
//!
//! Every VPS endpoint helper in this module group goes through [`client`]
//! so headers, timeouts, and the ngrok bypass header stay consistent. The
//! base URL always resolves to the production Hetzner backend
//! (`VPS_PROD_URL`) — for both debug and release builds — unless overridden
//! at runtime via `SIGNING_SERVICE_URL` or `BACKEND_URL` (checked in that
//! order), e.g. `SIGNING_SERVICE_URL=http://127.0.0.1:8090 cargo run` to
//! develop against a local backend instead.

use std::sync::RwLock;

const VPS_PROD_URL: &str = "https://xfchess.com";

/// The current backend JWT, set after wallet login (SIWS/bridge). When present,
/// it is sent as `Authorization: Bearer …` on every VPS call, which is the
/// preferred (per-user) auth for the session-key signing endpoints.
static AUTH_TOKEN: RwLock<Option<String>> = RwLock::new(None);

/// Store (or clear) the backend JWT used for VPS requests. Call with `Some(jwt)`
/// after login and `None` on logout.
pub fn set_auth_token(token: Option<String>) {
    if let Ok(mut guard) = AUTH_TOKEN.write() {
        *guard = token;
    }
}

pub fn vps_base() -> String {
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| VPS_PROD_URL.to_string())
}

/// Same resolution as [`vps_base`] but as a `ws(s)://` URL, for websocket
/// endpoints (e.g. `/ws/auth`).
pub fn vps_ws_base() -> String {
    let base = vps_base();
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base
    }
}

fn default_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        "ngrok-skip-browser-warning",
        reqwest::header::HeaderValue::from_static("true"),
    );
    // Preferred per-user auth: a backend JWT obtained after wallet login.
    if let Ok(guard) = AUTH_TOKEN.read() {
        if let Some(token) = guard.as_ref() {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
                h.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
    }
    // Legacy fallback for the VPS session-key signing endpoints
    // (/move/record, /session/*, /game/finalize, …): a shared relay
    // secret matching the backend's RELAY_SHARED_SECRET. Sent alongside
    // the JWT during the dual-accept rollout; harmless once retired.
    if let Ok(secret) = std::env::var("RELAY_SHARED_SECRET") {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&secret) {
            h.insert("X-Relay-Secret", value);
        }
    }
    h
}

/// For endpoints that do real on-chain work server-side before responding
/// (`/session/activate` funds + cosigns + confirms two transactions, each
/// with its own 30s confirmation deadline — see `backend/src/signing/solana/
/// transactions.rs`) and genuinely need headroom past a normal request.
pub fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .default_headers(default_headers())
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// For endpoints that are a plain DB/in-memory lookup or a fast instruction
/// build with no on-chain RPC in the request path — session creation,
/// registration-info lookups, tx-building, status checks. These should
/// never take more than a couple of seconds under normal conditions; a
/// blocked or unreachable backend should surface as a fast, clear error
/// (see `LobbyStatus::Error`'s "Try Again") instead of leaving the caller
/// stuck on a spinner for up to two minutes waiting on `client()`'s much
/// longer timeout, which is tuned for the on-chain-work endpoints instead.
pub fn client_fast() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .default_headers(default_headers())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

// ── Tauri wallet bridge port resolution ─────────────────────────────────────
//
// Pure local port-file/env-var lookup — no Solana SDK dependency — so it
// lives here rather than under `multiplayer::solana` (which is gated behind
// the `solana` cargo feature). It was previously defined in
// `multiplayer::solana::tauri_signer`, which broke every no-`solana`-feature
// build the moment a caller outside that module (main_menu.rs's wallet-bridge
// status poller, itself feature-independent) referenced it unconditionally.

fn nominal_wallet_bridge_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

/// Path the Tauri bridge's HTTP server writes its actual bound port to.
/// Must match `http_bridge_port_file()` in `tauri/src/main.rs`. The bridge
/// tries the nominal port first and only falls back to scanning upward if
/// that's already taken (e.g. a second local instance with no
/// XFCHESS_WALLET_PORT override) — without reading this file back, every
/// HTTP call this client makes to the bridge would still target the
/// nominal port even when the bridge itself is actually listening
/// somewhere else, which looked identical to the bridge not running at all.
fn http_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-http-{}.port",
        nominal_wallet_bridge_port()
    ))
}

/// HTTP port for this instance's Tauri wallet bridge: the bridge's
/// announced actual port if its port file is present and parses, else the
/// nominal XFCHESS_WALLET_PORT-derived value (default 7454) — matching the
/// common single-instance case where they're always the same.
pub fn wallet_bridge_port() -> u16 {
    if let Some(port) = std::env::var("XFCHESS_ACTUAL_WALLET_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        return port;
    }

    std::fs::read_to_string(http_bridge_port_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(nominal_wallet_bridge_port)
}

/// Backend response payload for the cached multi-currency rate endpoint
/// (`GET /api/rates/all`), narrowed to the USD figures — USD is the primary
/// display/input currency across the game client and admin panel.
#[derive(Debug, Clone)]
pub struct SolUsdRateResponse {
    /// SOL purchasable for 1 USD.
    pub sol_per_usd: f64,
    /// USD per 1 SOL.
    pub usd_per_sol: f64,
    /// Unix timestamp when the rate was fetched.
    pub fetched_at: i64,
}

/// Fetch the cached live SOL/USD rate from the signing backend's
/// `/api/rates/all` (same cache the admin panel and tournament creation use).
pub fn fetch_sol_usd_rate() -> Result<SolUsdRateResponse, String> {
    let resp = client()?
        .get(format!("{}/api/rates/all", vps_base()))
        .send()
        .map_err(|e| format!("vps fetch_sol_usd_rate: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps fetch_sol_usd_rate: HTTP {status} - {body}"));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("vps fetch_sol_usd_rate parse: {e}"))?;

    let usd_per_sol = json["rates"]["usd"]
        .as_f64()
        .ok_or("vps fetch_sol_usd_rate: missing rates.usd")?;
    let sol_per_usd = json["sol_per_fiat"]["usd"]
        .as_f64()
        .unwrap_or(if usd_per_sol > 0.0 {
            1.0 / usd_per_sol
        } else {
            0.0
        });
    let fetched_at = json["fetched_at"].as_i64().unwrap_or(0);

    Ok(SolUsdRateResponse {
        sol_per_usd,
        usd_per_sol,
        fetched_at,
    })
}
