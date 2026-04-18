//! HTTP route handlers for Solana Blinks API.
//!
//! This module provides Axum route handlers for the Blinks endpoints
//! following the Solana Action specification.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::info;

use crate::signing::{
    AppState,
    blinks::{
        ActionMetadata, 
        RegisterTransactionRequest, 
        TransactionResponse, 
        ValidationResult, 
        BalanceResult,
        get_action_metadata,
        build_register_transaction,
        validate_registration,
        check_wallet_balance,
    },
    blinks_chains::{ActionChain, create_registration_chain, create_onboarding_chain},
};

/// Creates the Blinks API router.
pub fn blinks_routes() -> Router<AppState> {
    Router::new()
        .route("/tournament/:id", get(get_tournament_action))
        .route("/tournament/:id/register", post(register_transaction))
        .route("/tournament/:id/check-balance", get(check_balance))
        .route("/tournament/:id/validate", post(validate_registration_endpoint))
        .route("/tournament/:id/chain/registration", get(get_registration_chain))
        .route("/tournament/:id/chain/onboarding", get(get_onboarding_chain))
}

/// GET /api/actions/tournament/:id
///
/// Returns Action metadata for a tournament.
async fn get_tournament_action(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<ActionMetadata>, StatusCode> {
    let program_id = std::str::FromStr::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let metadata = get_action_metadata(id, &state.tournament_store, &program_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(Json(metadata))
}

/// POST /api/actions/tournament/:id/register
///
/// Builds a registration transaction for the player.
async fn register_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RegisterTransactionRequest>,
) -> Result<Json<TransactionResponse>, StatusCode> {
    let wallet_pubkey = solana_sdk::pubkey::Pubkey::from_str(&req.account)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let program_id = std::str::FromStr::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let tx_response = build_register_transaction(
        id,
        &wallet_pubkey,
        &state.tournament_store,
        &program_id,
        &state.feepayer.next(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to build transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    info!("[blinks] Built registration transaction for tournament {} wallet {}", id, req.account);
    
    Ok(Json(tx_response))
}

/// GET /api/actions/tournament/:id/check-balance?wallet=<pubkey>
///
/// Checks if a wallet has sufficient SOL balance for registration.
async fn check_balance(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<BalanceResult>, StatusCode> {
    let wallet_str = params.get("wallet").ok_or(StatusCode::BAD_REQUEST)?;
    let wallet_pubkey = solana_sdk::pubkey::Pubkey::from_str(wallet_str)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let balance = check_wallet_balance(&wallet_pubkey, id, &state.tournament_store)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check balance: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(balance))
}

/// POST /api/actions/tournament/:id/validate
///
/// Validates a player for tournament registration (anti-cheat checks).
async fn validate_registration_endpoint(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ValidationResult>, StatusCode> {
    let wallet_str = body.get("account")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let wallet_pubkey = solana_sdk::pubkey::Pubkey::from_str(wallet_str)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let ip_address = body.get("ip_address")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let validation = validate_registration(
        id,
        &wallet_pubkey,
        &state.tournament_store,
        &state.elo_cache,
        &state.identity_vault,
        ip_address,
    )
    .await
    .map_err(|e| {
        tracing::error!("Validation failed: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    info!("[blinks] Validation for tournament {} wallet {}: valid={}", 
        id, wallet_str, validation.valid);
    
    Ok(Json(validation))
}

/// GET /api/actions/tournament/:id/chain/registration
///
/// Returns the registration action chain for users with existing wallets and SOL.
async fn get_registration_chain(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ActionChain>, StatusCode> {
    let wallet = params.get("wallet").map(|s| s.clone());
    
    let tournament = state.tournament_store.get(id).await
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let required_sol = tournament.entry_fee_lamports as f64 / 1_000_000_000.0;
    
    if wallet.is_none() {
        // No wallet provided, return onboarding chain instead
        let chain = create_onboarding_chain(id, None, required_sol);
        return Ok(Json(chain));
    }
    
    let chain = create_registration_chain(id, &wallet.unwrap());
    Ok(Json(chain))
}

/// GET /api/actions/tournament/:id/chain/onboarding
///
/// Returns the onboarding action chain for users without wallets or SOL.
async fn get_onboarding_chain(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ActionChain>, StatusCode> {
    let wallet = params.get("wallet").map(|s| s.clone());
    
    let tournament = state.tournament_store.get(id).await
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let required_sol = tournament.entry_fee_lamports as f64 / 1_000_000_000.0;
    
    let chain = create_onboarding_chain(id, wallet, required_sol);
    Ok(Json(chain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let router = blinks_routes();
        assert!(router.not_found("test").is_some());
    }
}
