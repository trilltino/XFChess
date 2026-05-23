//! Shared HTTP client configuration for VPS calls.
//!
//! Every VPS endpoint helper in this module group goes through [`client`]
//! so headers, timeouts, and the ngrok bypass header stay consistent. The
//! base URL comes from `SIGNING_SERVICE_URL` (runtime) or `BACKEND_URL`
//! (compile-time), defaulting to local dev.

use serde::Deserialize;

const VPS_PROD_URL: &str = "http://178.104.55.19";
const VPS_LOCAL_URL: &str = "http://127.0.0.1:8090";

pub fn vps_base() -> String {
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| {
            // Debug builds default to local backend; release builds hit production.
            if cfg!(debug_assertions) {
                VPS_LOCAL_URL.to_string()
            } else {
                VPS_PROD_URL.to_string()
            }
        })
}

pub(crate) fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "ngrok-skip-browser-warning",
                reqwest::header::HeaderValue::from_static("true"),
            );
            h
        })
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Backend response payload for the live SOL/GBP rate endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct SolGbpRateResponse {
    /// SOL purchasable for 1 GBP.
    pub sol_per_gbp: f64,
    /// GBP per 1 SOL.
    pub gbp_per_sol: f64,
    /// Unix timestamp when the rate was fetched.
    pub fetched_at: i64,
}

/// Fetch the cached live SOL/GBP rate from the signing backend.
pub fn fetch_sol_gbp_rate() -> Result<SolGbpRateResponse, String> {
    let resp = client()?
        .get(format!("{}/api/rates/sol-gbp", vps_base()))
        .send()
        .map_err(|e| format!("vps fetch_sol_gbp_rate: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps fetch_sol_gbp_rate: HTTP {status} - {body}"));
    }

    resp.json::<SolGbpRateResponse>()
        .map_err(|e| format!("vps fetch_sol_gbp_rate parse: {e}"))
}
