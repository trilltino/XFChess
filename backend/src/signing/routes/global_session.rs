//! Global persistent session key endpoints.
//!
//! These routes let the game client/VPS:
//!
//! - `POST /global-session/prepare` — Generate a new session keypair,
//!   return its pubkey + the `authorize_global_session` transaction for the
//!   wallet to sign.  The private key is stored in-memory (keyed by wallet
//!   pubkey) until the wallet confirms.
//!
//! - `POST /global-session/activate` — Receive the signed transaction,
//!   broadcast it, then keep the session keypair as the live signer for this
//!   wallet.
//!
//! - `DELETE /global-session/:wallet` — Revoke: broadcast
//!   `revoke_global_session` and remove the keypair from memory.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, post},
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::signing::AppState;

/// A `prepare`d session older than this with no matching `activate` is
/// abandoned (wallet never signed) — drop it instead of leaking the
/// keypair in memory for the life of the process.
const PENDING_SESSION_TTL: Duration = Duration::from_secs(600);
const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// Periodically evicts stale `prepare`d-but-never-`activate`d sessions.
pub fn spawn_pending_session_sweep(pending: Arc<Mutex<HashMap<Pubkey, (Keypair, Instant)>>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            let mut sessions = pending.lock().await;
            let before = sessions.len();
            sessions.retain(|_, (_, prepared_at)| prepared_at.elapsed() < PENDING_SESSION_TTL);
            let removed = before - sessions.len();
            if removed > 0 {
                info!("[global_session] swept {removed} abandoned pending session(s)");
            }
        }
    });
}

// ── Route registration ────────────────────────────────────────────────────────

/// Public: game client calls this to check whether a session exists.
pub fn global_session_public_routes() -> Router<AppState> {
    Router::new().route("/{wallet}/verify", axum::routing::get(verify))
}

/// Protected: require admin API key — prepare/activate/revoke mutate server-held keypairs.
pub fn global_session_protected_routes() -> Router<AppState> {
    Router::new()
        .route("/prepare", post(prepare))
        .route("/activate", post(activate))
        .route("/{wallet}", delete(revoke))
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PrepareReq {
    pub wallet_pubkey: String,
    /// Lamports to deposit into the session vault (default 0.1 SOL).
    #[serde(default = "default_deposit")]
    pub deposit_lamports: u64,
}
fn default_deposit() -> u64 {
    100_000_000
} // 0.1 SOL

#[derive(Serialize)]
pub struct PrepareResp {
    /// The session key pubkey the client should store.
    pub session_pubkey: String,
    /// The `GlobalSessionDelegation` PDA.
    pub session_pda: String,
    /// Base64-encoded unsigned transaction for the wallet to sign + submit.
    pub tx_b64: String,
}

#[derive(Deserialize)]
pub struct ActivateReq {
    pub wallet_pubkey: String,
    /// Base64-encoded signed transaction (wallet-signed `authorize_global_session`).
    pub signed_tx_b64: String,
}

#[derive(Serialize)]
pub struct ActivateResp {
    pub sig: String,
    pub session_pubkey: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Generate a fresh session keypair, derive the PDA, build an unsigned
/// `authorize_global_session` transaction, and return it to the client.
async fn prepare(
    State(state): State<AppState>,
    Json(req): Json<PrepareReq>,
) -> Result<Json<PrepareResp>, (StatusCode, String)> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;

    let session_kp = Keypair::new();
    let session_pubkey = session_kp.pubkey();

    let program_id = state.program_id;
    let (session_pda, _bump) =
        Pubkey::find_program_address(&[b"global_session", wallet.as_ref()], &program_id);

    // Build unsigned authorize_global_session instruction
    let discriminator: [u8; 8] = [0x15, 0xd3, 0x8a, 0x6c, 0xf2, 0x71, 0x4e, 0xb2];
    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(session_pubkey.as_ref()); // session_key field
    data.push(0u8); // duration_secs: None
    data.push(0u8); // spending_limit: None
    data.push(0u8); // max_wager: None
    data.push(0u8); // games: None
    data.extend_from_slice(&req.deposit_lamports.to_le_bytes());

    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(session_pda, false),
        solana_sdk::instruction::AccountMeta::new(wallet, true),
        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    let ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts,
        data,
    };

    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let (recent_blockhash, _) = tokio::task::spawn_blocking(move || {
        rpc.get_latest_blockhash_with_commitment(
            solana_sdk::commitment_config::CommitmentConfig::confirmed(),
        )
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::BAD_GATEWAY, format!("RPC: {e}")))?;

    let tx = Transaction::new_with_payer(&[ix], Some(&wallet));
    // Unsigned — wallet must sign before broadcasting.
    let _ = recent_blockhash; // blockhash will be set when wallet signs
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {e}")))?;
    let tx_b64 = general_purpose::STANDARD.encode(tx_bytes);

    // Store ephemeral keypair in AppState pending-sessions map
    {
        let mut sessions = state.pending_global_sessions.lock().await;
        sessions.insert(wallet, (session_kp, Instant::now()));
    }

    info!("global_session prepare: wallet={wallet} session={session_pubkey}");
    Ok(Json(PrepareResp {
        session_pubkey: session_pubkey.to_string(),
        session_pda: session_pda.to_string(),
        tx_b64,
    }))
}

/// Receive the signed transaction, broadcast it, promote the pending session
/// to the active global session for this wallet.
async fn activate(
    State(state): State<AppState>,
    Json(req): Json<ActivateReq>,
) -> Result<Json<ActivateResp>, (StatusCode, String)> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid pubkey: {e}")))?;

    let tx_bytes = general_purpose::STANDARD
        .decode(&req.signed_tx_b64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("base64: {e}")))?;
    let tx: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("deserialize tx: {e}")))?;

    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let sig = tokio::task::spawn_blocking(move || rpc.send_and_confirm_transaction(&tx))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("RPC send: {e}")))?;

    // Promote from pending → active
    let session_kp = {
        let mut sessions = state.pending_global_sessions.lock().await;
        sessions.remove(&wallet).map(|(kp, _prepared_at)| kp)
    };
    let session_kp = match session_kp {
        Some(kp) => kp,
        None => {
            warn!("global_session activate: no pending session for {wallet}");
            return Err((StatusCode::NOT_FOUND, "no pending session".into()));
        }
    };

    let session_pubkey = session_kp.pubkey().to_string();
    {
        let mut active = state.active_global_sessions.lock().await;
        active.insert(wallet, session_kp);
    }

    info!("global_session activated: wallet={wallet} sig={sig}");
    Ok(Json(ActivateResp {
        sig: sig.to_string(),
        session_pubkey,
    }))
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
    {
        let mut pending = state.pending_global_sessions.lock().await;
        pending.remove(&wallet);
    }

    info!("global_session revoked (in-memory): wallet={wallet}");
    Ok(Json(serde_json::json!({
        "status": "revoked",
        "session_pda": session_pda.to_string(),
    })))
}
