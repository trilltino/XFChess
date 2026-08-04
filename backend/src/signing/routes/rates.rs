//! Exchange rate endpoints for fiat-crypto conversion.
//!
//! Provides cached SOL rates for multiple fiat currencies (USD, GBP, EUR, CAD, BRL)
//! so the frontend can display accurate wager tiers and dashboard metrics.

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::telemetry::worker_metrics;

/// A SOL/USD price outside this band is virtually certainly a bug (bad
/// decimal/expo parsing, a corrupted upstream feed) rather than reality —
/// reject rather than let it flow into a real fee calculation.
const SOL_USD_SANITY_MIN: f64 = 1.0;
const SOL_USD_SANITY_MAX: f64 = 10_000.0;

/// Flat platform fee charged per wagered game: 10p per player × 2 players.
/// The single source of truth for this figure — both `create_session` (the
/// legacy per-game path) and `get_platform_fee` (the global-session path,
/// which has no session-creation round-trip to piggyback the fee on) convert
/// it via the same `RateCache::gbp_to_lamports` call so it can't drift
/// between the two flows.
pub const PLATFORM_FEE_GBP: f64 = 0.20;

/// Relative difference between the primary and secondary sources' SOL/USD
/// rate above which we log+count the disagreement as an anomaly worth
/// investigating. Two independent live-quote providers normally agree
/// within a fraction of a percent.
const DIVERGENCE_ALERT_THRESHOLD: f64 = 0.03;

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
    /// Convert a GBP amount to lamports using the live SOL/GBP rate.
    /// Returns `None` if the rate is unavailable.
    pub async fn gbp_to_lamports(&self, gbp: f64) -> Option<u64> {
        let rates = self.get().await.ok()?;
        let gbp_per_sol = rates.get("gbp")?;
        if *gbp_per_sol <= 0.0 {
            return None;
        }
        let sol_amount = gbp / gbp_per_sol;
        Some((sol_amount * 1_000_000_000.0).round() as u64)
    }

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

        // Slow path: fetch both sources independently. Primary (Helius/CoinGecko
        // SOL/USD × frankfurter.app FX) is preferred; secondary (CoinGecko's own
        // direct multi-currency pricing, a genuinely different computation path)
        // is both a real fallback and a cross-check on the primary.
        let (primary, secondary) =
            tokio::join!(fetch_primary_rates(), fetch_secondary_rates_coingecko());
        let primary = primary.and_then(validate_rates);
        let secondary = secondary.and_then(validate_rates);

        if let (Ok(p), Ok(s)) = (&primary, &secondary) {
            check_divergence(p, s);
        }

        let resolved = match (primary, secondary) {
            (Ok(p), _) => Ok(p),
            (Err(e_primary), Ok(s)) => {
                error!(
                    "[RATES] Primary source unavailable ({}), using secondary",
                    e_primary
                );
                Ok(s)
            }
            (Err(e_primary), Err(e_secondary)) => {
                worker_metrics::RATES_FETCH_FAILED_TOTAL.fetch_add(1, Ordering::Relaxed);
                Err(format!("primary: {e_primary}; secondary: {e_secondary}"))
            }
        };

        match resolved {
            Ok(rates) => {
                let cached = CachedRates {
                    rates: rates.clone(),
                    fetched_at: Instant::now(),
                };
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

    /// Refreshes the cache on a fixed interval regardless of request traffic,
    /// so payment-critical call sites (tournament/session creation) never
    /// block on a live external fetch and a fetch outage is caught proactively
    /// via `[RATES]` error logs / the `xfchess_rates_fetch_failed_total` metric
    /// instead of surfacing as a slow or failed player-facing request.
    pub fn spawn_background_refresh(&self) {
        let cache = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(cache.ttl);
            ticker.tick().await; // skip immediate first tick — the first real request primes the cache
            loop {
                ticker.tick().await;
                if let Err(e) = cache.get().await {
                    error!("[RATES] Background refresh failed: {}", e);
                }
            }
        });
    }
}

/// Rejects a rate map whose SOL/USD figure falls outside a plausible band —
/// catches a parsing bug or a broken upstream feed before it reaches a real
/// fee calculation. Also rejects non-finite or non-positive values.
fn validate_rates(rates: HashMap<String, f64>) -> Result<HashMap<String, f64>, String> {
    let sol_usd = *rates
        .get("usd")
        .ok_or_else(|| "missing usd rate".to_string())?;
    if !sol_usd.is_finite() || !(SOL_USD_SANITY_MIN..=SOL_USD_SANITY_MAX).contains(&sol_usd) {
        worker_metrics::RATES_SANITY_REJECTED_TOTAL.fetch_add(1, Ordering::Relaxed);
        return Err(format!(
            "sol_usd {sol_usd} outside sanity bounds [{SOL_USD_SANITY_MIN}, {SOL_USD_SANITY_MAX}]"
        ));
    }
    for (currency, rate) in &rates {
        if !rate.is_finite() || *rate <= 0.0 {
            worker_metrics::RATES_SANITY_REJECTED_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(format!(
                "{currency} rate {rate} is non-finite or non-positive"
            ));
        }
    }
    Ok(rates)
}

/// Logs (and counts) a wide disagreement between the two independent sources.
/// Deliberately does not pick a side — auto-resolving a disagreement is its
/// own risk vector; the safe response to "our sources disagree" is a paged
/// human, with the trusted primary still served in the meantime.
fn check_divergence(primary: &HashMap<String, f64>, secondary: &HashMap<String, f64>) {
    for (currency, p) in primary {
        let Some(s) = secondary.get(currency) else {
            continue;
        };
        if *p == 0.0 {
            continue;
        }
        let relative_diff = (p - s).abs() / p;
        if relative_diff > DIVERGENCE_ALERT_THRESHOLD {
            worker_metrics::RATES_SOURCE_DIVERGENCE_TOTAL.fetch_add(1, Ordering::Relaxed);
            error!(
                "[RATES] Source divergence on {}: primary={} secondary={} ({:.1}% apart, threshold {:.1}%)",
                currency,
                p,
                s,
                relative_diff * 100.0,
                DIVERGENCE_ALERT_THRESHOLD * 100.0
            );
        }
    }
}

/// Primary source: SOL/USD from Helius (or CoinGecko fallback), converted to
/// each fiat currency via frankfurter.app FX rates. Two-hop chain.
async fn fetch_primary_rates() -> Result<HashMap<String, f64>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        // CoinGecko's public API 403s any request without a descriptive
        // User-Agent (reqwest sends none by default).
        .user_agent("XFChess-Backend/1.0 (+https://xfchess.com)")
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

    info!(
        "[RATES] Fetched SOL rates via Helius+Frankfurter: {:?}",
        rates
    );
    Ok(rates)
}

/// Secondary source: CoinGecko's own direct multi-currency pricing in a
/// single call. Independent of the primary's two-hop (SOL/USD × FX) chain —
/// genuine redundancy, not just a fallback that shares a failure mode with
/// the primary's own CoinGecko-as-SOL/USD-fallback path.
async fn fetch_secondary_rates_coingecko() -> Result<HashMap<String, f64>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("XFChess-Backend/1.0 (+https://xfchess.com)")
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let resp = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd,gbp,eur,cad,brl")
        .send()
        .await
        .map_err(|e| format!("CoinGecko multi-currency: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("CoinGecko multi-currency error {status}: {body}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("CoinGecko multi-currency json: {e}"))?;

    let solana_obj = json
        .get("solana")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "CoinGecko multi-currency: missing solana object".to_string())?;

    let mut rates = HashMap::new();
    for (currency, value) in solana_obj {
        if let Some(rate) = value.as_f64() {
            rates.insert(currency.to_lowercase(), rate);
        }
    }

    info!(
        "[RATES] Fetched secondary SOL rates via CoinGecko direct: {:?}",
        rates
    );
    Ok(rates)
}

/// Fetch SOL/USD spot price from Helius token-price API.
/// Skips straight to the CoinGecko fallback when HELIUS_API_KEY is unset —
/// never ship a hardcoded key in source.
async fn fetch_sol_usd_helius(client: &reqwest::Client) -> Result<f64, String> {
    // Try Helius first when a key is configured (never hardcode a key in source);
    // otherwise go straight to the CoinGecko fallback below.
    if let Ok(api_key) = std::env::var("HELIUS_API_KEY") {
        if !api_key.is_empty() {
            let url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": "sol-price",
                "method": "getAsset",
                "params": { "id": "So11111111111111111111111111111111111111112" }
            });

            let helius_result: Result<f64, String> = async {
                let resp = client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Helius RPC: {e}"))?;
                let json: serde_json::Value =
                    resp.json().await.map_err(|e| format!("Helius json: {e}"))?;
                json.pointer("/result/token_info/price_info/price_per_token")
                    .and_then(|p| p.as_f64())
                    .ok_or_else(|| "Helius RPC: no price_per_token".to_string())
            }
            .await;

            if let Ok(price) = helius_result {
                return Ok(price);
            }
        }
    }

    // Fallback: CoinGecko public (no key, rate-limited but always available)
    let cg_resp = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
        .send()
        .await
        .map_err(|e| format!("CoinGecko: {e}"))?;
    let cg_json: serde_json::Value = cg_resp
        .json()
        .await
        .map_err(|e| format!("CoinGecko json: {e}"))?;
    cg_json
        .pointer("/solana/usd")
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

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Frankfurter json: {e}"))?;

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
) -> axum::response::Response {
    match app_state.rate_cache.get().await {
        Ok(rates) => {
            let mut sol_per_fiat = HashMap::new();
            for (currency, rate) in &rates {
                sol_per_fiat.insert(currency.clone(), 1.0 / rate);
            }

            Json(ExchangeRatesResponse {
                rates,
                sol_per_fiat,
                fetched_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            })
            .into_response()
        }
        Err(e) => {
            error!("[RATES] Failed to fetch rates: {}", e);
            // Surface the underlying fetch error in the body — callers (the
            // game client, this admin panel) log the response body verbatim
            // on a non-2xx status, so this is the only way to see *why* the
            // upstream fetch failed without direct access to this process's
            // own console.
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
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

/// Response payload for /api/rates/platform-fee.
#[derive(Serialize)]
pub struct PlatformFeeResponse {
    pub platform_fee_lamports: u64,
}

/// GET /api/rates/platform-fee — the flat per-game platform fee (`PLATFORM_FEE_GBP`),
/// converted to lamports at the live SOL/GBP rate. Lets the global-session
/// create-game path charge the same fee the legacy path already does via
/// `create_session`, without needing to create a per-game backend session
/// just to learn the number.
async fn get_platform_fee(
    State(app_state): State<crate::signing::AppState>,
) -> Result<Json<PlatformFeeResponse>, StatusCode> {
    let platform_fee_lamports = app_state
        .rate_cache
        .gbp_to_lamports(PLATFORM_FEE_GBP)
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(PlatformFeeResponse {
        platform_fee_lamports,
    }))
}

/// Builds the rates router (no auth required — public rate feed).
/// State is provided by the parent router's `.with_state(AppState)`.
pub fn rates_routes() -> Router<crate::signing::AppState> {
    Router::new()
        .route("/all", get(get_all_rates))
        .route("/sol-gbp", get(get_sol_gbp_rate))
        .route("/platform-fee", get(get_platform_fee))
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
