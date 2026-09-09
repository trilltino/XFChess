use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, post},
    Router,
};
use serde::Deserialize;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

use crate::signing::auth::RequireWallet;
use crate::signing::AppState;

// ── Route registration ────────────────────────────────────────────────────────

pub fn global_session_public_routes() -> Router<AppState> {
    Router::new().route("/{wallet}/verify", axum::routing::get(verify))
}

pub fn global_session_protected_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/track-game", post(track_game))
        .route("/{wallet}", delete(revoke))
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterReq {
    pub wallet_pubkey: String,
    pub session_secret_key_b58: String,
}

async fn register(
    State(state): State<AppState>,
    caller: RequireWallet,
    Json(req): Json<RegisterReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    caller.require_is(&req.wallet_pubkey)?;

    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;
    let secret_bytes = bs58::decode(&req.session_secret_key_b58)
        .into_vec()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid secret key: {e}")))?;
    let session_kp = Keypair::try_from(secret_bytes.as_slice())
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid keypair: {e}")))?;

    let (session_pda, _bump) =
        Pubkey::find_program_address(&[b"global_session", wallet.as_ref()], &state.program_id);
    let rpc = Arc::clone(&state.solana_rpc);
    let data = tokio::task::spawn_blocking(move || rpc.get_account_data(&session_pda))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("no GlobalSessionDelegation found on-chain for {wallet}: {e}"),
            )
        })?;

    // GlobalSessionDelegation layout: disc(8) + player(32) + session_key(32) + ...
    let on_chain_session_key = data
        .get(40..72)
        .and_then(|b| Pubkey::try_from(b).ok())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "malformed GlobalSessionDelegation account".to_string(),
        ))?;

    if on_chain_session_key != session_kp.pubkey() {
        warn!(
            "global_session register: key mismatch for {wallet} (submitted {}, on-chain {})",
            session_kp.pubkey(),
            on_chain_session_key
        );
        return Err((
            StatusCode::FORBIDDEN,
            "submitted key does not match the on-chain authorized session key".to_string(),
        ));
    }

    let session_pubkey = session_kp.pubkey().to_string();
    {
        let mut active = state.active_global_sessions.lock().await;
        active.insert(wallet, session_kp);
    }

    info!(
        "global_session registered (client-authorized): wallet={wallet} session={session_pubkey}"
    );
    Ok(Json(serde_json::json!({
        "session_pubkey": session_pubkey,
    })))
}

#[derive(Deserialize)]
pub struct TrackGameReq {
    pub game_id: u64,
    pub wallet_pubkey: String,
}

async fn track_game(
    State(state): State<AppState>,
    caller: RequireWallet,
    Json(req): Json<TrackGameReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Without this the route copied any named wallet's global session keypair
    // into `sessions[game_id]` for a caller-chosen `game_id`, with no
    // authentication whatsoever — the first half of a chain that ended in that
    // wallet's key signing an attacker-supplied transaction.
    caller.require_is(&req.wallet_pubkey)?;

    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;

    let keypair_bytes = {
        let sessions = state.active_global_sessions.lock().await;
        match sessions.get(&wallet) {
            Some(kp) => kp.to_bytes(),
            None => {
                warn!("global_session track_game: no registered session for {wallet}, skipping");
                return Ok(StatusCode::ACCEPTED);
            }
        }
    };

    state
        .store
        .create_with_keypair(req.game_id, wallet, keypair_bytes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!(
        "global_session track_game: game {} tracked for wallet {wallet}",
        req.game_id
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn verify(
    State(state): State<AppState>,
    Path(wallet_str): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let wallet = Pubkey::from_str(&wallet_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    let active = state.active_global_sessions.lock().await;
    if let Some(kp) = active.get(&wallet) {
        Ok(Json(serde_json::json!({
            "active": true,
            "session_pubkey": kp.pubkey().to_string(),
        })))
    } else {
        Ok(Json(serde_json::json!({
            "active": false,
        })))
    }
}

async fn revoke(
    State(state): State<AppState>,
    caller: RequireWallet,
    Path(wallet_str): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    caller.require_is(&wallet_str)?;

    let wallet = Pubkey::from_str(&wallet_str)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;

    let (session_pda, _) =
        Pubkey::find_program_address(&[b"global_session", wallet.as_ref()], &state.program_id);

    {
        let mut active = state.active_global_sessions.lock().await;
        active.remove(&wallet);
    }

    info!("global_session revoked (in-memory): wallet={wallet}");
    Ok(Json(serde_json::json!({
        "status": "revoked_locally",
        "session_pda": session_pda.to_string(),
        "note": "The on-chain delegation is still active. Sign and submit \
                 revoke_global_session with this session_pda to revoke it there too.",
    })))
}
