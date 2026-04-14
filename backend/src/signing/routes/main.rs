//! Main API routes for the XFChess signing service.
//!
//! This module provides HTTP endpoints for:
//! - Authentication (JWT issuance)
//! - Session management (create, activate, status)
//! - Move recording (via Execution Rollup)
//! - Game lifecycle (undelegate, finalize)
//! - Transaction signing (session key delegation)
//! - Statistics (active games, players)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use tracing::{error, info, warn};
use sha2::{Digest, Sha256};

use crate::signing::{AppState, solana};

// ── Request / Response types ─────────────────────────────────────────────────

/// Request to create a new game session.
#[derive(Deserialize)]
pub struct CreateSessionReq {
    pub game_id: u64,
    pub wallet_pubkey: String,
}

/// Response containing the session public key.
#[derive(Serialize)]
pub struct CreateSessionResp {
    pub session_pubkey: String,
}

/// Request to activate a session with wallet-signed transaction.
#[derive(Deserialize)]
pub struct ActivateSessionReq {
    pub game_id: u64,
    /// Base64-encoded signed Transaction bytes (wallet-signed create/join + authorize_session_key)
    pub signed_tx_b64: String,
}

/// Request to record a chess move.
#[derive(Deserialize)]
pub struct RecordMoveReq {
    pub game_id: u64,
    pub move_uci: String,
    pub next_fen: String,
    #[serde(default)]
    pub nonce: u64,
}

/// Response containing transaction signature.
#[derive(Serialize)]
pub struct SigResp {
    pub sig: String,
}

/// Request to sign a transaction with session key.
#[derive(Deserialize)]
pub struct SignReq {
    pub game_id: u64,
    /// Base64-encoded serialized Transaction that needs the session key signature
    pub tx_b64: String,
}

/// Creates the main API routes router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/issue", post(issue_jwt))
        .route("/session/create", post(create_session))
        .route("/session/activate", post(activate_session))
        .route("/session/status/:game_id", get(session_status))
        .route("/session/sign", post(sign_tx))
        .route("/move/record", post(record_move))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
        .route("/stats", get(get_stats))
}

// ── Auth ──────────────────────────────────────────────────────────────────────

/// POST /auth/issue - Issues a JWT token for a wallet.
/// MVP: issues JWT unconditionally (add challenge-response later).
pub async fn issue_jwt(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let wallet = body
        .get("wallet_pubkey")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let token = state.jwt.issue(wallet).map_err(|e| {
        error!("JWT issue error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(serde_json::json!({ "jwt": token })))
}

// ── Session ───────────────────────────────────────────────────────────────────

/// POST /session/create - Creates a new session for a game.
pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, StatusCode> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let session_pubkey = state.store.create(req.game_id, wallet).await;
    info!("[VPS] Created session for game {} → {}", req.game_id, session_pubkey);
    Ok(Json(CreateSessionResp { session_pubkey: session_pubkey.to_string() }))
}

/// POST /session/activate - Activates a session with wallet-signed setup TX.
pub async fn activate_session(
    State(state): State<AppState>,
    Json(req): Json<ActivateSessionReq>,
) -> Result<Json<SigResp>, StatusCode> {
    // Idempotency: if the session is already active the TX already landed.
    // Return success without re-submitting to avoid GameAlreadyFull on retries.
    if state.store.is_active(req.game_id).await {
        info!("[VPS] Session for game {} already active — idempotent success", req.game_id);
        return Ok(Json(SigResp { sig: "already-active".to_string() }));
    }

    let tx_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.signed_tx_b64,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;

    let rpc = solana::make_rpc(&state.config.solana_rpc_url);

    // Submit the wallet-signed TX (create_game/join_game + authorize_session_key)
    let sig = solana::submit_signed_tx(&rpc, &tx_bytes).map_err(|e| {
        error!("[VPS] Failed to submit setup TX for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] Setup TX confirmed for game {}: {sig}", req.game_id);

    // Mark session active
    state.store.activate(req.game_id).await;

    // Fund session key from fee-payer pool
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let fee_payer = state.feepayer.next();
    const FUND_LAMPORTS: u64 = 10_000_000; // 0.01 SOL covers ~2000 TXs
    if let Err(e) = solana::fund_account(&rpc, fee_payer, &entry.session_pubkey(), FUND_LAMPORTS) {
        warn!("[VPS] Could not fund session key for game {}: {e}", req.game_id);
    }

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// GET /session/status/:game_id - Gets session status.
pub async fn session_status(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.store.get(game_id).await {
        Some(entry) => Ok(Json(serde_json::json!({
            "active": entry.active,
            "session_pubkey": entry.session_pubkey().to_string(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ── Moves ─────────────────────────────────────────────────────────────────────

/// POST /move/record - Records a move on the Execution Rollup.
pub async fn record_move(
    State(state): State<AppState>,
    Json(req): Json<RecordMoveReq>,
) -> Result<Json<SigResp>, StatusCode> {
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if !entry.active {
        return Err(StatusCode::PRECONDITION_FAILED);
    }

    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session_pk = entry.session_pubkey();
    let session_kp = entry.keypair();

    // Internal signing for data-level replay protection
    // Signature = sign(session_key, hash(game_id, move_uci, next_fen, nonce))
    let mut hasher = Sha256::new();
    hasher.update(req.game_id.to_le_bytes());
    hasher.update(req.move_uci.as_bytes());
    hasher.update(req.next_fen.as_bytes());
    hasher.update(req.nonce.to_le_bytes());
    let hash = hasher.finalize();
    let sig_bytes = session_kp.sign_message(&hash).as_ref().to_vec();

    let ix = solana::record_move_ix(
        &program_id,
        &session_pk,
        &entry.wallet_pubkey,
        req.game_id,
        &req.move_uci,
        &req.next_fen,
        req.nonce,
        Some(sig_bytes),
    );
    let er_rpc = solana::make_rpc(&state.config.er_rpc_url);

    let sig = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] record_move failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] record_move game {} move {} sig {}", req.game_id, req.move_uci, sig);

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// POST /game/undelegate - Undelegates a game from ER to devnet.
pub async fn undelegate_game(
    State(state): State<AppState>,
    Json(req): Json<UndelegateGameReq>,
) -> Result<Json<SigResp>, StatusCode> {
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session_kp = entry.keypair();
    let session_pk = entry.session_pubkey();

    let ix = solana::undelegate_game_ix(&program_id, &session_pk, req.game_id);
    let er_rpc = solana::make_rpc(&state.config.er_rpc_url);

    let sig = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] undelegate_game failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] undelegate_game game {} sig {}", req.game_id, sig);

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// POST /game/finalize - Finalizes a game on devnet.
pub async fn finalize_game(
    State(state): State<AppState>,
    Json(req): Json<FinalizeGameReq>,
) -> Result<Json<SigResp>, StatusCode> {
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session_kp = entry.keypair();

    let white = Pubkey::from_str(&req.white_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let black = Pubkey::from_str(&req.black_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let winner = req.winner.as_deref();

    let ix = solana::finalize_game_ix(&program_id, req.game_id, &white, &black, winner);
    let rpc = solana::make_rpc(&state.config.solana_rpc_url);

    let sig = solana::sign_and_submit(&rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] finalize_game failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] finalize_game game {} winner={:?} sig {}", req.game_id, req.winner, sig);

    Ok(Json(SigResp { sig: sig.to_string() }))
}

#[derive(Deserialize)]
pub struct UndelegateGameReq {
    pub game_id: u64,
}

#[derive(Deserialize)]
pub struct FinalizeGameReq {
    pub game_id: u64,
    pub winner: Option<String>,   // "white" | "black" | null (draw)
    pub white_pubkey: String,
    pub black_pubkey: String,
}

/// POST /session/sign - Signs a transaction with the session key.
pub async fn sign_tx(
    State(state): State<AppState>,
    Json(req): Json<SignReq>,
) -> Result<Json<SigResp>, StatusCode> {
    use solana_sdk::transaction::Transaction;

    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;

    let tx_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.tx_b64,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut tx: Transaction = bincode::deserialize(&tx_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let session_kp = entry.keypair();

    let rpc = solana::make_rpc(&state.config.solana_rpc_url);
    let blockhash = rpc.get_latest_blockhash().map_err(|_| StatusCode::BAD_GATEWAY)?;
    tx.partial_sign(&[&session_kp], blockhash);

    let sig = rpc.send_and_confirm_transaction(&tx).map_err(|e| {
        error!("[VPS] sign_tx failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(SigResp { sig: sig.to_string() }))
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatsResp {
    pub active_games: u64,
    pub unique_players: u64,
    pub total_sessions: u64,
    pub uptime_seconds: u64,
}

/// GET /stats - Global platform statistics.
pub async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResp>, StatusCode> {
    let active_games = state.store.count_active().await;
    let unique_players = state.store.count_unique_players().await;
    let total_sessions = state.store.count_total_sessions().await;

    // Simple uptime tracking (could be improved with proper resource)
    let uptime_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(Json(StatsResp {
        active_games,
        unique_players,
        total_sessions,
        uptime_seconds,
    }))
}
