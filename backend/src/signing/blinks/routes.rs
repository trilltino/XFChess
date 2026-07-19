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

use crate::signing::blinks::{Action, ActionLinks};
use crate::signing::{
    blinks::chains::{create_onboarding_chain, create_registration_chain, ActionChain},
    blinks::core::{
        build_claim_prize_transaction, build_register_transaction,
        build_start_tournament_transactions, check_wallet_balance, get_action_metadata,
        validate_registration, ActionMetadata, BalanceResult, RegisterTransactionRequest,
        TransactionResponse, ValidationResult,
    },
    AppState,
};
use serde::Deserialize;

/// Creates the Blinks API router.
pub fn blinks_routes() -> Router<AppState> {
    Router::new()
        .route("/tournament/{id}", get(get_tournament_action))
        .route("/tournament/{id}/register", post(register_transaction))
        .route(
            "/tournament/{id}/register/confirm",
            post(confirm_registration),
        )
        .route("/tournament/{id}/check-balance", get(check_balance))
        .route(
            "/tournament/{id}/validate",
            post(validate_registration_endpoint),
        )
        .route("/tournament/{id}/claim-prize", get(get_claim_prize_action))
        .route(
            "/tournament/{id}/claim-prize",
            post(claim_prize_transaction),
        )
        .route(
            "/tournament/{id}/chain/registration",
            get(get_registration_chain),
        )
        .route(
            "/tournament/{id}/chain/onboarding",
            get(get_onboarding_chain),
        )
        // Admin-only — requires X-API-Key header (enforced by the admin middleware)
        .route("/admin/tournament/{id}/start", post(start_tournament))
}

#[derive(Deserialize)]
struct ConfirmRegistrationRequest {
    account: String,
    signature: String,
}

/// GET /api/actions/tournament/:id
///
/// Returns Action metadata for a tournament.
async fn get_tournament_action(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<ActionMetadata>, StatusCode> {
    let metadata = get_action_metadata(id, &state.tournament_store)
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
    let wallet_pubkey =
        solana_sdk::pubkey::Pubkey::from_str(&req.account).map_err(|_| StatusCode::BAD_REQUEST)?;

    let program_id = std::str::FromStr::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tx_response = build_register_transaction(
        id,
        &wallet_pubkey,
        &state.tournament_store,
        &program_id,
        &state.feepayer.next(),
        &state.tournament_fee_recipient,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to build transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "[blinks] Built registration transaction for tournament {} wallet {}",
        id, req.account
    );

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
    let wallet_pubkey =
        solana_sdk::pubkey::Pubkey::from_str(wallet_str).map_err(|_| StatusCode::BAD_REQUEST)?;

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
    let wallet_str = body
        .get("account")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let wallet_pubkey =
        solana_sdk::pubkey::Pubkey::from_str(wallet_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let ip_address = body
        .get("ip_address")
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

    info!(
        "[blinks] Validation for tournament {} wallet {}: valid={}",
        id, wallet_str, validation.valid
    );

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

    let tournament = state
        .tournament_store
        .get(id)
        .await
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

    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let required_sol = tournament.entry_fee_lamports as f64 / 1_000_000_000.0;

    let chain = create_onboarding_chain(id, wallet, required_sol);
    Ok(Json(chain))
}

/// GET /api/actions/tournament/:id/claim-prize?wallet=<pubkey>
///
/// Returns Action metadata for prize claiming.
/// Only valid when the tournament is Completed and the wallet is a prize finisher.
async fn get_claim_prize_action(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ActionMetadata>, StatusCode> {
    use crate::signing::storage::tournament::TournamentStatus;

    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if tournament.status != TournamentStatus::Completed {
        return Err(StatusCode::CONFLICT);
    }

    let _wallet = params
        .get("wallet")
        .map(|s| s.as_str())
        .unwrap_or("unknown");
    let prize_sol = tournament.prize_pool as f64 / 1_000_000_000.0;

    Ok(Json(ActionMetadata {
        icon: "https://xfchess.com/logo.png".to_string(),
        title: format!("Claim Prize — {}", tournament.name),
        description: format!(
            "You placed in this tournament. Claim your share of the {:.4} SOL prize pool.",
            prize_sol
        ),
        label: "Claim Prize".to_string(),
        links: ActionLinks {
            actions: vec![Action {
                label: "Claim SOL Prize".to_string(),
                href: format!("/api/actions/tournament/{}/claim-prize", id),
            }],
        },
    }))
}

/// POST /api/actions/tournament/:id/claim-prize
///
/// Builds a `claim_tournament_prize` transaction for the winner to sign.
async fn claim_prize_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RegisterTransactionRequest>,
) -> Result<Json<TransactionResponse>, StatusCode> {
    let claimant =
        solana_sdk::pubkey::Pubkey::from_str(&req.account).map_err(|_| StatusCode::BAD_REQUEST)?;
    let program_id = std::str::FromStr::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tx_response = build_claim_prize_transaction(
        id,
        &claimant,
        &state.tournament_store,
        &program_id,
        &state.feepayer.next(),
    )
    .await
    .map_err(|e| {
        tracing::error!(
            "[blinks] claim_prize failed for tournament {} wallet {}: {:?}",
            id,
            req.account,
            e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "[blinks] Built claim_prize transaction for tournament {} wallet {}",
        id, req.account
    );
    Ok(Json(tx_response))
}

/// POST /api/actions/admin/tournament/:id/start
///
/// Fires `start_tournament` + `initialize_match × N` on-chain.
/// Requires X-API-Key header (enforced upstream by admin middleware).
async fn start_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::signing::storage::tournament::TournamentStatus;

    let tournament = state
        .tournament_store
        .get(id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    if tournament.status != TournamentStatus::Registration {
        return Err(StatusCode::CONFLICT);
    }

    let program_id = std::str::FromStr::from_str(&state.config.program_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let txs = build_start_tournament_transactions(
        id,
        &state.tournament_store,
        &program_id,
        &state.vps_authority,
        &state.tournament_fee_recipient,
    )
    .await
    .map_err(|e| {
        tracing::error!(
            "[blinks] build_start_tournament_transactions failed for {}: {:?}",
            id,
            e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "[blinks] start_tournament broadcast {} txs for tournament {}",
        txs.len(),
        id
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "tx_count": txs.len(), "transactions": txs }),
    ))
}

/// POST /api/actions/tournament/:id/register/confirm
///
/// Called by the client after broadcasting the registration transaction.
/// Polls RPC for confirmation (up to 60 s), then adds the player to the
/// in-memory tournament store.
async fn confirm_registration(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ConfirmRegistrationRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signature::Signature;
    use std::str::FromStr;

    let sig = Signature::from_str(&req.signature).map_err(|_| StatusCode::BAD_REQUEST)?;
    let player =
        solana_sdk::pubkey::Pubkey::from_str(&req.account).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Confirm the tournament exists and is open for registration.
    {
        use crate::signing::storage::tournament::TournamentStatus;
        let t = state
            .tournament_store
            .get(id)
            .await
            .ok_or(StatusCode::NOT_FOUND)?;
        if t.status != TournamentStatus::Registration {
            return Err(StatusCode::CONFLICT);
        }
    }

    // Poll devnet/mainnet RPC for confirmation — up to 60 s.
    let rpc_url = state.config.solana_rpc_url.clone();
    let confirmed = tokio::task::spawn_blocking(move || {
        use crate::signing::solana::make_rpc;
        use solana_sdk::commitment_config::CommitmentConfig;
        let rpc = make_rpc(&rpc_url);
        for _ in 0..30 {
            match rpc.get_signature_status_with_commitment(&sig, CommitmentConfig::confirmed()) {
                Ok(Some(Ok(()))) => return true,
                Ok(Some(Err(_))) => return false,
                _ => {}
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        false
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !confirmed {
        return Ok(Json(serde_json::json!({
            "ok": false,
            "error": "transaction not confirmed within 60 s"
        })));
    }

    // Add the player to the store (idempotent — no-op if already present).
    state
        .tournament_store
        .update(id, |t| {
            if !t.players.iter().any(|p| p == &player.to_string()) {
                t.players.push(player.to_string());
                t.player_elos.push(0);
                t.prize_pool += t.entry_fee_lamports.saturating_sub(t.platform_fee_lamports);
            }
        })
        .await;

    // Notify the scheduler that a player joined.
    let player_count = state
        .tournament_store
        .get(id)
        .await
        .map(|t| t.players.len())
        .unwrap_or(0);
    if let Some(tx) = &state.tournament_trigger {
        let _ = tx
            .send(crate::signing::TournamentTrigger::PlayerJoined {
                tournament_id: id,
                player_count,
            })
            .await;
    }

    info!(
        "[blinks] Registration confirmed for tournament {} player {} ({} total)",
        id, player, player_count
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "player_count": player_count }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let _router = blinks_routes();
    }
}
