//! Global persistent session key endpoints.
//!
//! These routes let the game client/VPS:
//!
//! - `DELETE /global-session/:wallet` — Revoke: broadcast
//!   `revoke_global_session` and remove the keypair from memory.
//!
//! - `POST /global-session/register` — The game client's authorization path
//!   (`authorize_global_session_if_needed`) generates the session keypair and
//!   signs+submits `authorize_global_session` on-chain itself, then hands the
//!   backend a copy of that already-authorized key so it can still act on the
//!   player's behalf for `finalize_game`/`undelegate_game`/settlement — the
//!   same custody model the original per-game flow already uses, just keyed
//!   by wallet instead of by game. Verified against on-chain state before
//!   being trusted.

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

use crate::signing::AppState;

// ── Route registration ────────────────────────────────────────────────────────

/// Public: game client calls this to check whether a session exists.
pub fn global_session_public_routes() -> Router<AppState> {
    Router::new().route("/{wallet}/verify", axum::routing::get(verify))
}

/// Player-facing: no shared secret required — `revoke` only takes effect
/// on-chain with a validly wallet-signed transaction, same trust model as the
/// per-game `/session/create` + `/session/activate` routes.
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
    /// Base58-encoded raw session keypair bytes (64 bytes, ed25519 secret +
    /// public) — the client already generated this and used it to sign
    /// `authorize_global_session` on-chain itself.
    pub session_secret_key_b58: String,
}

/// POST /global-session/register — accept a session key the client
/// authorized entirely on its own (see module docs). Not blindly trusted:
/// the derived pubkey must match the `GlobalSessionDelegation.session_key`
/// already recorded on-chain for this wallet — a field only that wallet's
/// own signature could have set — before it's stored. A mismatched or
/// bogus key is rejected outright.
async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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

/// POST /global-session/track-game — tell the backend a game was created via
/// the global-session flow, so `settlement_worker`'s scan loop (which only
/// knows to look at `SessionStore` rows) can discover and auto-settle it.
/// `finalize_game`/`undelegate_game`/`session_status` don't need this call
/// at all — they resolve a signer straight from on-chain `fee_payer` (see
/// `routes::main::resolve_game_signer`) — this exists purely so the
/// *proactive* scan has something to iterate. Requires the wallet to already
/// have a `register`ed session; silently a no-op otherwise (nothing to track
/// yet, the client will just have to call `/game/finalize` itself in that
/// case rather than being auto-settled).
async fn track_game(
    State(state): State<AppState>,
    Json(req): Json<TrackGameReq>,
) -> Result<StatusCode, (StatusCode, String)> {
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

/// GET /global-session/:wallet/verify
/// Returns whether the VPS holds an active global session for this wallet,
/// along with the session pubkey if it does. The client calls this at MainMenu
/// entry to decide whether to show the "Authorize session" banner.
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

/// Broadcast `revoke_global_session` and remove the session from memory.
async fn revoke(
    State(state): State<AppState>,
    Path(wallet_str): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let wallet = Pubkey::from_str(&wallet_str)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;

    let (session_pda, _) =
        Pubkey::find_program_address(&[b"global_session", wallet.as_ref()], &state.program_id);

    // The revoke instruction is wallet-signed — we only need to tell the client
    // the accounts it must include. The actual revoke tx is built client-side.
    {
        let mut active = state.active_global_sessions.lock().await;
        active.remove(&wallet);
    }

    info!("global_session revoked (in-memory): wallet={wallet}");
    Ok(Json(serde_json::json!({
        "status": "revoked",
        "session_pda": session_pda.to_string(),
    })))
}
