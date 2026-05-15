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
    /// Get the current rates, fetching from CoinGecko if stale/missing.
    pub async fn get(&self) -> Result<HashMap<String, f64>, String> {
        // Fast path: read lock
        {
            let read = self.inner.read().await;
            if let Some(ref cached) = *read {
                if cached.is_fresh(self.ttl) {
                    return Ok(cached.rates.clone());
                }
            }
        }

        // Slow path: fetch and write
        let rates = fetch_sol_rates_from_coingecko().await?;
        let cached = CachedRates {
            rates: rates.clone(),
            fetched_at: Instant::now(),
        };
        *self.inner.write().await = Some(cached);
        Ok(rates)
    }
}

/// Fetch SOL rates from CoinGecko public API (no API key required for simple price).
async fn fetch_sol_rates_from_coingecko() -> Result<HashMap<String, f64>, String> {
    const URL: &str = "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=gbp,brl,eur,cad,usd";

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get(URL)
        .send()
        .await
        .map_err(|e| format!("http send: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("CoinGecko error {status}: {body}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("json parse: {e}"))?;

    let solana_rates = json
        .get("solana")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "missing solana object in CoinGecko response".to_string())?;

    let mut rates = HashMap::new();
    for (currency, value) in solana_rates {
        if let Some(rate) = value.as_f64() {
            rates.insert(currency.to_string(), rate);
        }
    }

    info!("[RATES] Fetched SOL rates: {:?}", rates);
    Ok(rates)
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
    State(cache): State<RateCache>,
) -> Result<Json<ExchangeRatesResponse>, StatusCode> {
    match cache.get().await {
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
    State(cache): State<RateCache>,
) -> Result<Json<SolGbpResponse>, StatusCode> {
    match cache.get().await {
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
pub fn rates_routes() -> Router<RateCache> {
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
