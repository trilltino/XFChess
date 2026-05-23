//! Wallet balance endpoint — fetches SOL + SPL stablecoin balances via Helius RPC.
//!
//! GET /api/wallet/balance/:pubkey
//!
//! Returns live SOL balance converted to USD and local fiat using cached Pyth rates,
//! plus balances for supported stablecoins (USDC, EURC, BRLA).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::warn;

// ── Stablecoin mint addresses (Solana mainnet) ──────────────────────────────
// Verify at: https://solana.fm/address/<mint> before deploying to mainnet
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const EURC_MINT: &str = "HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr"; // Circle EURC — verify on mainnet
const BRLA_MINT: &str = "BRLbKUMNMHhpSA6pppwJT6MLBpVtCVhpPAMHu9KMLVH"; // BRLA Digital — verify on mainnet

// ── Helius RPC helpers ──────────────────────────────────────────────────────

fn helius_rpc_url() -> String {
    let key = std::env::var("HELIUS_API_KEY")
        .unwrap_or_else(|_| "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5".to_string());
    format!("https://mainnet.helius-rpc.com/?api-key={key}")
}

fn helius_beta_url() -> String {
    let key = std::env::var("HELIUS_API_KEY")
        .unwrap_or_else(|_| "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5".to_string());
    format!("https://beta.helius-rpc.com/?api-key={key}")
}

#[derive(Deserialize)]
struct RpcResponse<T> {
    result: T,
}

#[derive(Deserialize)]
struct BalanceResult {
    value: u64,
}

#[derive(Deserialize)]
struct TokenAccountsResult {
    value: Vec<TokenAccountEntry>,
}

#[derive(Deserialize)]
struct TokenAccountEntry {
    account: TokenAccountData,
}

#[derive(Deserialize)]
struct TokenAccountData {
    data: TokenParsedData,
}

#[derive(Deserialize)]
struct TokenParsedData {
    parsed: TokenParsedInfo,
}

#[derive(Deserialize)]
struct TokenParsedInfo {
    info: TokenInfo,
}

#[derive(Deserialize)]
struct TokenInfo {
    #[serde(rename = "tokenAmount")]
    token_amount: TokenAmount,
    mint: String,
}

#[derive(Deserialize)]
struct TokenAmount {
    #[serde(rename = "uiAmount")]
    ui_amount: Option<f64>,
}

/// Fetch SOL balance (lamports) for a pubkey via Helius RPC.
async fn fetch_sol_lamports(client: &reqwest::Client, pubkey: &str) -> Result<u64, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [pubkey, {"commitment": "confirmed"}]
    });

    let resp = client
        .post(helius_rpc_url())
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Helius getBalance: {e}"))?;

    if !resp.status().is_success() {
        // Fall back to beta endpoint
        let beta_resp = client
            .post(helius_beta_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Helius beta getBalance: {e}"))?;
        let rpc: RpcResponse<BalanceResult> = beta_resp
            .json()
            .await
            .map_err(|e| format!("Helius beta json: {e}"))?;
        return Ok(rpc.result.value);
    }

    let rpc: RpcResponse<BalanceResult> = resp
        .json()
        .await
        .map_err(|e| format!("Helius json: {e}"))?;
    Ok(rpc.result.value)
}

/// Fetch SPL token balances for a given owner via Helius RPC (parsed encoding).
async fn fetch_token_balances(
    client: &reqwest::Client,
    pubkey: &str,
) -> Result<Vec<TokenAccountEntry>, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getTokenAccountsByOwner",
        "params": [
            pubkey,
            {"programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"},
            {"encoding": "jsonParsed", "commitment": "confirmed"}
        ]
    });

    let resp = client
        .post(helius_rpc_url())
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Helius getTokenAccountsByOwner: {e}"))?;

    let rpc: RpcResponse<TokenAccountsResult> = resp
        .json()
        .await
        .map_err(|e| format!("Helius token accounts json: {e}"))?;
    Ok(rpc.result.value)
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StablecoinBalances {
    /// USDC balance (USD)
    pub usdc: f64,
    /// EURC balance (EUR)
    pub eurc: f64,
    /// BRLA balance (BRL)
    pub brla: f64,
}

#[derive(Serialize)]
pub struct WalletBalanceResponse {
    pub pubkey: String,
    /// Raw SOL (not lamports)
    pub sol_balance: f64,
    /// SOL value in USD at current Pyth rate
    pub usd_value: f64,
    /// SOL value in the player's local currency
    pub local_value: f64,
    /// Three-letter currency code for local_value
    pub local_currency: String,
    /// Currency symbol for display (£, €, R$, C$, $)
    pub local_symbol: String,
    /// SPL stablecoin balances
    pub stablecoins: StablecoinBalances,
}

// ── Route handler ───────────────────────────────────────────────────────────

/// GET /api/wallet/balance/:pubkey?country=GB
///
/// `country` is optional — falls back to USD if omitted or unrecognised.
async fn get_wallet_balance(
    Path(pubkey): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<crate::signing::AppState>,
) -> Result<Json<WalletBalanceResponse>, StatusCode> {
    let country = params.get("country").map(|s| s.as_str()).unwrap_or("US");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| {
            warn!("[WALLET] HTTP client build failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Fetch SOL balance
    let lamports = fetch_sol_lamports(&client, &pubkey).await.map_err(|e| {
        warn!("[WALLET] SOL balance fetch failed for {pubkey}: {e}");
        StatusCode::BAD_GATEWAY
    })?;
    let sol_balance = lamports as f64 / 1_000_000_000.0;

    // Convert to USD using Pyth rates
    let rates = state.pyth_oracle.get_rates().unwrap_or_else(|| {
        warn!("[WALLET] Pyth rates unavailable, using zero rates");
        Default::default()
    });

    let usd_value = sol_balance * rates.sol_usd;
    let (local_value, local_currency) = rates.usd_to_local(usd_value, country);
    let local_symbol = crate::signing::pyth_oracle::FxRates::symbol(local_currency).to_string();

    // Fetch SPL token balances
    let token_accounts = fetch_token_balances(&client, &pubkey).await.unwrap_or_default();

    let mut usdc_bal = 0.0f64;
    let mut eurc_bal = 0.0f64;
    let mut brla_bal = 0.0f64;

    for entry in &token_accounts {
        let mint = &entry.account.data.parsed.info.mint;
        let amount = entry.account.data.parsed.info.token_amount.ui_amount.unwrap_or(0.0);
        if mint == USDC_MINT {
            usdc_bal += amount;
        } else if mint == EURC_MINT {
            eurc_bal += amount;
        } else if mint == BRLA_MINT {
            brla_bal += amount;
        }
    }

    Ok(Json(WalletBalanceResponse {
        pubkey,
        sol_balance,
        usd_value,
        local_value,
        local_currency: local_currency.to_string(),
        local_symbol,
        stablecoins: StablecoinBalances {
            usdc: usdc_bal,
            eurc: eurc_bal,
            brla: brla_bal,
        },
    }))
}

/// Builds the wallet router.
pub fn wallet_routes() -> Router<crate::signing::AppState> {
    Router::new().route("/balance/{pubkey}", get(get_wallet_balance))
}
