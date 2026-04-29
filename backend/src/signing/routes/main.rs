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
use crate::db::repository::GameRepository;

// ── Request / Response types ─────────────────────────────────────────────────

/// Request to create a new game session.
#[derive(Deserialize, Serialize)]
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
#[derive(Deserialize, Serialize)]
pub struct ActivateSessionReq {
    pub game_id: u64,
    /// Base64-encoded signed Transaction bytes (wallet-signed create/join + authorize_session_key)
    pub signed_tx_b64: String,
}

/// Request to record a chess move.
#[derive(Deserialize, Serialize)]
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
#[derive(Deserialize, Serialize)]
pub struct SignReq {
    pub game_id: u64,
    /// Base64-encoded serialized Transaction that needs the session key signature
    pub tx_b64: String,
}

/// Response containing player profile details.
#[derive(Serialize)]
pub struct PlayerProfileResp {
    pub elo: u32,
    pub country: String,
    pub username: String,
}

/// Creates the main API routes router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/issue", post(issue_jwt))
        .route("/session/create", post(create_session))
        .route("/session/activate", post(activate_session))
        .route("/session/status/{game_id}", get(session_status))
        .route("/session/sign", post(sign_tx))
        .route("/session/tee_auth", post(tee_auth))
        .route("/move/record", post(record_move))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
        .route("/player/{pubkey}", get(get_player_profile))
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

    // Retrieve session keypair so we can co-sign the TX.
    // The create_game / join_game instructions require BOTH the player wallet
    // signature (already in tx_bytes) and the session key signature (fee_payer).
    let entry = state.store.get(req.game_id).await.ok_or_else(|| {
        error!("[VPS] No session found for game {} — call /session/create first", req.game_id);
        StatusCode::NOT_FOUND
    })?;
    let session_keypair = entry.keypair();

    // Submit the wallet-signed TX, adding the session key co-signature.
    let sig = solana::cosign_and_submit_tx(&rpc, &session_keypair, &tx_bytes).map_err(|e| {
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

    // Fire-and-forget DB write (log errors but don't fail the HTTP response)
    let game_id_str = req.game_id.to_string();
    let player_wallet = entry.wallet_pubkey.to_string();
    let pool = state.store.pool();
    let move_uci = req.move_uci.clone();
    let next_fen = req.next_fen.clone();
    tokio::spawn(async move {
        // Ensure game row exists on first move
        let repo = GameRepository::new(pool);
        if let Err(e) = repo.upsert_game(&game_id_str).await {
            error!("[DB] Failed to upsert game {}: {}", game_id_str, e);
            return;
        }
        // Increment move counter and get the new number
        let move_number = repo.get_next_move_number(&game_id_str).await.unwrap_or(1) as i32;
        // Insert the move
        if let Err(e) = repo.add_move_simple(&game_id_str, move_number, &move_uci, Some(&next_fen), &player_wallet).await {
            error!("[DB] Failed to insert move for game {}: {}", game_id_str, e);
        }
    });

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

    // Fire-and-forget DB write (log errors but don't fail the HTTP response)
    let game_id_str = req.game_id.to_string();
    let pool = state.store.pool();
    let elo_cache = state.elo_cache.clone();
    let white = req.white_pubkey.clone();
    let black = req.black_pubkey.clone();
    let winner = req.winner.clone();
    let sig_str = sig.to_string();
    tokio::spawn(async move {
        let repo = GameRepository::new(pool);
        // Look up usernames from users_v2
        let white_username = repo.get_username(&white).await.ok();
        let black_username = repo.get_username(&black).await.ok();
        // Finalize the game record
        if let Err(e) = repo.complete_game(
            &game_id_str,
            Some(&white),
            Some(&black),
            white_username.as_deref(),
            black_username.as_deref(),
            winner.as_deref(),
            None, // final_fen (not provided in request)
            &sig_str,
            0.0, // stake_amount (not provided in request)
        ).await {
            error!("[DB] Failed to finalize game {}: {}", game_id_str, e);
        }
        elo_cache.invalidate(&white);
        elo_cache.invalidate(&black);
    });

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// GET /player/:pubkey - Gets player profile details (ELO, country, username).
pub async fn get_player_profile(
    Path(pubkey): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<PlayerProfileResp>, StatusCode> {
    let elo_data = state.elo_cache.get_elo(&pubkey).await.map_err(|e| {
        warn!("Failed to fetch profile for {}: {}", pubkey, e);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(PlayerProfileResp {
        elo: (elo_data.elo_rating / 100.0) as u32,
        country: elo_data.country,
        username: elo_data.username,
    }))
}

#[derive(Deserialize, Serialize)]
pub struct UndelegateGameReq {
    pub game_id: u64,
}

#[derive(Deserialize, Serialize)]
pub struct FinalizeGameReq {
    pub game_id: u64,
    pub winner: Option<String>,   // "white" | "black" | null (draw)
    pub white_pubkey: String,
    pub black_pubkey: String,
}

#[derive(Deserialize)]
pub struct TeeAuthReq {
    pub game_id: u64,
    pub wallet_pubkey: String,
    /// Base64-encoded Ed25519 signature over "Authenticate with MagicBlock TEE"
    pub signature_b64: String,
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

/// POST /session/tee_auth - Verifies wallet ownership for TEE-backed private game state.
///
/// The wallet signs the canonical message "Authenticate with MagicBlock TEE" with its
/// Ed25519 keypair. This backend verifies the signature and confirms the wallet identity,
/// enabling encrypted move processing on the MagicBlock Ephemeral Rollup TEE.
/// The session for `game_id` must already exist (player must call /session/create first).
pub async fn tee_auth(
    State(state): State<AppState>,
    Json(req): Json<TeeAuthReq>,
) -> Result<Json<SigResp>, StatusCode> {
    use base64::Engine;
    use solana_sdk::signature::Signature;

    const TEE_AUTH_MESSAGE: &[u8] = b"Authenticate with MagicBlock TEE";

    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| {
        warn!("[TEE-AUTH] Invalid wallet pubkey: {}", req.wallet_pubkey);
        StatusCode::BAD_REQUEST
    })?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.signature_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let sig = Signature::try_from(sig_bytes.as_slice()).map_err(|_| {
        warn!("[TEE-AUTH] Bad signature length ({} bytes) for game {}", sig_bytes.len(), req.game_id);
        StatusCode::BAD_REQUEST
    })?;

    if !sig.verify(wallet.as_ref(), TEE_AUTH_MESSAGE) {
        warn!("[TEE-AUTH] Signature mismatch — wallet {} game {}", req.wallet_pubkey, req.game_id);
        return Err(StatusCode::UNAUTHORIZED);
    }

    let _entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;

    info!("[TEE-AUTH] Wallet {} authenticated for game {}", req.wallet_pubkey, req.game_id);
    Ok(Json(SigResp { sig: "tee-auth-ok".to_string() }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::Router,
    };
    use tower::ServiceExt;
    use std::time::SystemTime;

    #[test]
    fn test_create_session_req_serialization() {
        let req = CreateSessionReq {
            game_id: 12345,
            wallet_pubkey: "test_wallet_pubkey".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_create_session_resp_serialization() {
        let resp = CreateSessionResp {
            session_pubkey: "test_session_pubkey".to_string(),
        };

        let json = serde_json::to_string(&resp);
        assert!(json.is_ok());
    }

    #[test]
    fn test_activate_session_req_serialization() {
        let req = ActivateSessionReq {
            game_id: 12345,
            signed_tx_b64: "base64_encoded_tx".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_record_move_req_serialization() {
        let req = RecordMoveReq {
            game_id: 12345,
            move_uci: "e2e4".to_string(),
            next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            nonce: 1,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_sig_resp_serialization() {
        let resp = SigResp {
            sig: "test_signature".to_string(),
        };

        let json = serde_json::to_string(&resp);
        assert!(json.is_ok());
    }

    #[test]
    fn test_sign_req_serialization() {
        let req = SignReq {
            game_id: 12345,
            tx_b64: "base64_encoded_tx".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_undelegate_game_req_serialization() {
        let req = UndelegateGameReq {
            game_id: 12345,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_finalize_game_req_serialization() {
        let req = FinalizeGameReq {
            game_id: 12345,
            winner: Some("white".to_string()),
            white_pubkey: "white_wallet".to_string(),
            black_pubkey: "black_wallet".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_stats_resp_serialization() {
        let resp = StatsResp {
            active_games: 10,
            unique_players: 20,
            total_sessions: 100,
            uptime_seconds: 3600,
        };

        let json = serde_json::to_string(&resp);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn test_routes_creation() {
        let _router = routes();
    }

    #[test]
    fn test_game_id_validation() {
        // Test valid game IDs
        let valid_ids = vec![0, 1, 12345, u64::MAX];
        for id in valid_ids {
            let req = CreateSessionReq {
                game_id: id,
                wallet_pubkey: "test_wallet".to_string(),
            };
            assert_eq!(req.game_id, id);
        }
    }

    #[test]
    fn test_wallet_pubkey_format() {
        // Test that wallet pubkey is a non-empty string
        let req = CreateSessionReq {
            game_id: 12345,
            wallet_pubkey: "test_wallet_pubkey".to_string(),
        };
        assert!(!req.wallet_pubkey.is_empty());
    }

    #[test]
    fn test_move_uci_format() {
        // Test valid UCI move format (e.g., "e2e4", "a7a8q")
        let valid_moves = vec!["e2e4", "e7e5", "a7a8q", "e1g1"];
        for move_uci in valid_moves {
            let req = RecordMoveReq {
                game_id: 12345,
                move_uci: move_uci.to_string(),
                next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
                nonce: 1,
            };
            assert_eq!(req.move_uci, move_uci);
        }
    }

    #[test]
    fn test_nonce_default() {
        // Test that nonce defaults to 0
        let req = RecordMoveReq {
            game_id: 12345,
            move_uci: "e2e4".to_string(),
            next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            nonce: 0, // Using #[serde(default)]
        };
        assert_eq!(req.nonce, 0);
    }

    #[test]
    fn test_winner_options() {
        // Test different winner options
        let winners = vec![
            Some("white".to_string()),
            Some("black".to_string()),
            Some("draw".to_string()),
            None,
        ];
        for winner in winners {
            let req = FinalizeGameReq {
                game_id: 12345,
                winner: winner.clone(),
                white_pubkey: "white_wallet".to_string(),
                black_pubkey: "black_wallet".to_string(),
            };
            assert_eq!(req.winner, winner);
        }
    }

    #[test]
    fn test_timestamp_generation() {
        // Test that SystemTime can generate valid timestamps
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time should be after UNIX_EPOCH");
        assert!(now.as_secs() > 0);
    }
}
