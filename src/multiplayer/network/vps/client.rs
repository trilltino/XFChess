use std::sync::RwLock;

const VPS_PROD_URL: &str = "https://xfchess.com";

static AUTH_TOKEN: RwLock<Option<String>> = RwLock::new(None);

pub fn set_auth_token(token: Option<String>) {
    if let Ok(mut guard) = AUTH_TOKEN.write() {
        *guard = token;
    }
}

pub fn logout() {
    let token = AUTH_TOKEN.read().ok().and_then(|guard| guard.clone());
    set_auth_token(None);
    if let Some(token) = token {
        if let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            let _ = client
                .post(format!("{}/api/auth/logout", vps_base()))
                .bearer_auth(token)
                .send();
        }
    }
}

pub fn vps_base() -> String {
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| VPS_PROD_URL.to_string())
}

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

pub fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .default_headers(default_headers())
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

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

fn http_bridge_port_file() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "xfchess-wallet-http-{}.port",
        nominal_wallet_bridge_port()
    ))
}

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

#[derive(Debug, Clone)]
pub struct SolUsdRateResponse {
    pub sol_per_usd: f64,
    pub usd_per_sol: f64,
    pub fetched_at: i64,
}

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
