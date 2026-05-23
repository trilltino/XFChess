//! Exchange rate endpoints for fiat-crypto conversion.
//!
//! Provides cached SOL rates for multiple fiat currencies (USD, GBP, EUR, CAD, BRL)
//! so the frontend can display accurate wager tiers and dashboard metrics.

use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info};

/// Cached rate entry with TTL.
#[derive(Clone, Debug)]
struct CachedRates {
    rates: HashMap<String, f64>,
    fetched_at: Instant,
}

impl CachedRates {
    fn is_fresh(&self, ttl: Duration) -> bool {
        self.fetched_at.elapsed() < ttl
    }
}

/// In-memory cache for SOL/Fiat rates (backend-process-local).
#[derive(Clone)]
pub struct RateCache {
    inner: Arc<RwLock<Option<CachedRates>>>,
    ttl: Duration,
}

impl Default for RateCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(60),
        }
    }
}

impl RateCache {
    /// Get the current rates. Returns stale cache on fetch failure rather than erroring.
    pub async fn get(&self) -> Result<HashMap<String, f64>, String> {
        // Fast path: fresh cache
        {
            let read = self.inner.read().await;
            if let Some(ref cached) = *read {
                if cached.is_fresh(self.ttl) {
                    return Ok(cached.rates.clone());
                }
            }
        }

        // Slow path: attempt fetch
        match fetch_sol_rates_from_coingecko().await {
            Ok(rates) => {
                let cached = CachedRates { rates: rates.clone(), fetched_at: Instant::now() };
                *self.inner.write().await = Some(cached);
                Ok(rates)
            }
            Err(e) => {
                // Return stale data rather than 503 — clients degrade gracefully on stale rates
                let read = self.inner.read().await;
                if let Some(ref stale) = *read {
                    error!("[RATES] Fetch failed ({}), serving stale rates", e);
                    return Ok(stale.rates.clone());
                }
                Err(e)
            }
        }
    }
}

/// Fetch SOL/USD from Helius, then convert via frankfurter.app FX rates.
async fn fetch_sol_rates_from_coingecko() -> Result<HashMap<String, f64>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let sol_usd = fetch_sol_usd_helius(&client).await?;
    let fx = fetch_usd_fx_rates(&client).await?;

    let mut rates = HashMap::new();
    rates.insert("usd".to_string(), sol_usd);
    for (currency, usd_per_unit) in &fx {
        // fx gives units-per-USD (e.g. GBP per 1 USD), so SOL/currency = SOL_USD * usd_per_unit
        rates.insert(currency.to_lowercase(), sol_usd * usd_per_unit);
    }

    info!("[RATES] Fetched SOL rates via Helius+Frankfurter: {:?}", rates);
    Ok(rates)
}

/// Fetch SOL/USD spot price from Helius token-price API.
async fn fetch_sol_usd_helius(client: &reqwest::Client) -> Result<f64, String> {
    let api_key = std::env::var("HELIUS_API_KEY")
        .unwrap_or_else(|_| "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5".to_string());
    // Use Helius DAS getAsset-style price endpoint (v1)
    let url = format!(
        "https://mainnet.helius-rpc.com/?api-key={}",
        api_key
    );
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "sol-price",
        "method": "getAsset",
        "params": { "id": "So11111111111111111111111111111111111111112" }
    });

    // Try Helius RPC first; fall back to CoinGecko public API if it fails
    let helius_result: Result<f64, String> = async {
        let resp = client.post(&url).json(&body).send().await
            .map_err(|e| format!("Helius RPC: {e}"))?;
        let json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Helius json: {e}"))?;
        json.pointer("/result/token_info/price_info/price_per_token")
            .and_then(|p| p.as_f64())
            .ok_or_else(|| "Helius RPC: no price_per_token".to_string())
    }.await;

    if let Ok(price) = helius_result {
        return Ok(price);
    }

    // Fallback: CoinGecko public (no key, rate-limited but always available)
    let cg_resp = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
        .send()
        .await
        .map_err(|e| format!("CoinGecko: {e}"))?;
    let cg_json: serde_json::Value = cg_resp.json().await
        .map_err(|e| format!("CoinGecko json: {e}"))?;
    cg_json.pointer("/solana/usd")
        .and_then(|p| p.as_f64())
        .ok_or_else(|| "CoinGecko: missing solana/usd".to_string())
}

/// Fetch USD FX rates from frankfurter.app (free, no key).
/// Returns a map of currency code (uppercase) → amount of that currency per 1 USD.
async fn fetch_usd_fx_rates(client: &reqwest::Client) -> Result<HashMap<String, f64>, String> {
    const URL: &str = "https://api.frankfurter.app/latest?from=USD&to=GBP,EUR,CAD,BRL";

    let resp = client
        .get(URL)
        .send()
        .await
        .map_err(|e| format!("Frankfurter request: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Frankfurter error {status}: {body}"));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("Frankfurter json: {e}"))?;

    let rates_obj = json
        .get("rates")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Frankfurter: missing rates object".to_string())?;

    let mut out = HashMap::new();
    for (k, v) in rates_obj {
        if let Some(rate) = v.as_f64() {
            out.insert(k.clone(), rate);
        }
    }
    Ok(out)
}

/// Response payload for /api/rates/all.
#[derive(Serialize)]
pub struct ExchangeRatesResponse {
    /// Map of currency code to its price per 1 SOL (e.g., {"usd": 150.5, "gbp": 120.2}).
    pub rates: HashMap<String, f64>,
    /// Map of currency code to SOL per 1 unit of fiat (reciprocal).
    pub sol_per_fiat: HashMap<String, f64>,
    /// Timestamp when rate was fetched (Unix seconds).
    pub fetched_at: i64,
}

/// GET /api/rates/all — cached SOL exchange rates for multiple currencies.
async fn get_all_rates(
    State(app_state): State<crate::signing::AppState>,
) -> Result<Json<ExchangeRatesResponse>, StatusCode> {
    match app_state.rate_cache.get().await {
        Ok(rates) => {
            let mut sol_per_fiat = HashMap::new();
            for (currency, rate) in &rates {
                sol_per_fiat.insert(currency.clone(), 1.0 / rate);
            }
            
            Ok(Json(ExchangeRatesResponse {
                rates,
                sol_per_fiat,
                fetched_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            }))
        }
        Err(e) => {
            error!("[RATES] Failed to fetch rates: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Legacy GET /api/rates/sol-gbp — cached SOL/GBP exchange rate (backward compatibility).
#[derive(Serialize)]
pub struct SolGbpResponse {
    pub sol_per_gbp: f64,
    pub gbp_per_sol: f64,
    pub fetched_at: i64,
}

async fn get_sol_gbp_rate(
    State(app_state): State<crate::signing::AppState>,
) -> Result<Json<SolGbpResponse>, StatusCode> {
    match app_state.rate_cache.get().await {
        Ok(rates) => {
            if let Some(&rate) = rates.get("gbp") {
                let sol_per_gbp = 1.0 / rate;
                Ok(Json(SolGbpResponse {
                    sol_per_gbp,
                    gbp_per_sol: rate,
                    fetched_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64,
                }))
            } else {
                Err(StatusCode::SERVICE_UNAVAILABLE)
            }
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Builds the rates router (no auth required — public rate feed).
/// State is provided by the parent router's `.with_state(AppState)`.
pub fn rates_routes() -> Router<crate::signing::AppState> {
    Router::new()
        .route("/all", get(get_all_rates))
        .route("/sol-gbp", get(get_sol_gbp_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_cache_starts_empty() {
        let cache = RateCache::default();
        let read = cache.inner.read().await;
        assert!(read.is_none());
    }

    #[test]
    fn test_cached_rate_freshness() {
        let mut rates = HashMap::new();
        rates.insert("usd".to_string(), 150.0);
        let cached = CachedRates {
            rates,
            fetched_at: Instant::now(),
        };
        assert!(cached.is_fresh(Duration::from_secs(60)));
        assert!(!cached.is_fresh(Duration::from_secs(0)));
    }
}
