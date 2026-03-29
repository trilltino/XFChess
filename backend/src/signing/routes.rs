use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use tracing::{error, info, warn};
use sha2::{Digest, Sha256};

use super::{AppState, solana};

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateSessionReq {
    pub game_id: u64,
    pub wallet_pubkey: String,
}

#[derive(Serialize)]
pub struct CreateSessionResp {
    pub session_pubkey: String,
}

#[derive(Deserialize)]
pub struct ActivateSessionReq {
    pub game_id: u64,
    /// Base64-encoded signed Transaction bytes (wallet-signed create/join + authorize_session_key).
    pub signed_tx_b64: String,
}

#[derive(Deserialize)]
pub struct RecordMoveReq {
    pub game_id: u64,
    pub move_uci: String,
    pub next_fen: String,
    #[serde(default)]
    pub nonce: u64,
}

#[derive(Serialize)]
pub struct SigResp {
    pub sig: String,
}

#[derive(Deserialize)]
pub struct SignReq {
    pub game_id: u64,
    /// Base64-encoded serialised Transaction that needs the session key signature.
    pub tx_b64: String,
}

// ── Auth ──────────────────────────────────────────────────────────────────────

/// POST /auth/issue  { wallet_pubkey }  → { jwt }
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

/// POST /session/create
pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, StatusCode> {
    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let session_pubkey = state.store.create(req.game_id, wallet).await;
    info!("[VPS] Created session for game {} → {}", req.game_id, session_pubkey);
    Ok(Json(CreateSessionResp { session_pubkey: session_pubkey.to_string() }))
}

/// POST /session/activate — submit wallet-signed setup TX, then fund session key.
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
    const FUND_LAMPORTS: u64 = 2_000_000; // 0.002 SOL covers ~400 TXs
    if let Err(e) = solana::fund_account(&rpc, fee_payer, &entry.session_pubkey(), FUND_LAMPORTS) {
        warn!("[VPS] Could not fund session key for game {}: {e}", req.game_id);
    }

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// GET /session/status/:game_id
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

/// POST /move/record — VPS builds + signs record_move instruction and submits to ER.
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

/// POST /game/undelegate — VPS signs + submits undelegate_game IX to the ER endpoint.
/// Commits the final ER state (moves) back to devnet and releases the accounts.
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

/// POST /game/finalize — VPS signs + submits finalize_game IX to devnet.
/// Must be called after undelegation has committed the ER state.
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

/// POST /session/sign — VPS adds session key signature to a pre-built TX and submits.
/// Used for delegation (client builds the complex ix, VPS signs with session key).
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
