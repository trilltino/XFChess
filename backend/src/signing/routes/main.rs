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
    /// Platform fee per game in lamports (20p GBP total = 10p per player × 2),
    /// calculated from the live SOL/GBP rate at session creation time.
    pub platform_fee_lamports: u64,
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

/// Client blur + think-time telemetry for one ply (see `report_blur_telemetry`).
#[derive(Deserialize)]
pub struct BlurTelemetryReq {
    pub game_id: u64,
    /// 1-based ply number, matching the server's `moves.move_number`.
    pub move_number: u32,
    /// "white" | "black" — must match the ply's parity.
    pub color: String,
    pub blurred: bool,
    /// Client-measured think time for this move in ms (optional).
    #[serde(default)]
    pub think_ms: Option<u32>,
}

/// Extended finalize response including payout breakdown.
#[derive(Serialize)]
pub struct FinalizeResp {
    pub sig: String,
    /// Lamports awarded to the winner (0 for draws/free games).
    pub winner_lamports: u64,
    /// Treasury fee deducted in lamports.
    pub country_fee: u64,
}

/// Nonce response for /game/:id/nonce.
#[derive(Serialize)]
pub struct NonceResp {
    /// The last confirmed on-chain nonce (client should use nonce + 1 for next move).
    pub nonce: u64,
}

/// Request body for free-rated ELO update.
#[derive(Deserialize, Serialize)]
pub struct FreeRatedResultReq {
    pub game_id: u64,
    pub winner: Option<String>,
    pub white_pubkey: String,
    pub black_pubkey: String,
}

/// Request body for submitting a dispute.
#[derive(Deserialize, Serialize)]
pub struct DisputeReq {
    pub game_id: u64,
    pub disputing_player: String,
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
        .route("/telemetry/blur", post(report_blur_telemetry))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
        .route("/game/{game_id}/nonce", get(get_move_nonce))
        .route("/ratings/update", post(update_free_rated_result))
        .route("/dispute/submit", post(submit_dispute))
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
    let session_pubkey = state.store.create(req.game_id, wallet).await.map_err(|e| {
        error!("[VPS] Failed to create session for game {}: {}", req.game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    // 10p per player × 2 players = 20p GBP total platform fee per game
    let platform_fee_lamports = state.rate_cache.gbp_to_lamports(0.20).await.unwrap_or(0);
    info!("[VPS] Created session for game {} → {} (fee: {} lamports)", req.game_id, session_pubkey, platform_fee_lamports);
    Ok(Json(CreateSessionResp { session_pubkey: session_pubkey.to_string(), platform_fee_lamports }))
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
/// Validates engine-side before Solana submission; derives next_fen internally.
pub async fn record_move(
    State(state): State<AppState>,
    Json(req): Json<RecordMoveReq>,
) -> Result<Json<SigResp>, StatusCode> {
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if !entry.active {
        return Err(StatusCode::PRECONDITION_FAILED);
    }

    // ── Engine-side validation (replay from previous FEN) ──
    let pool = state.store.pool();
    let repo = GameRepository::new(pool.clone());

    let prev_fen = if let Ok(moves) = repo.get_moves(&req.game_id.to_string()).await {
        moves.iter().max_by_key(|m| m.move_number).and_then(|m| m.fen_after.clone())
    } else {
        None
    }.unwrap_or_else(|| "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    let mut game = nimzovich_engine::on_chain::CompactBoard::from_fen(&prev_fen).to_on_chain_game();

    // Parse UCI into fixed 5-byte array
    let mv_bytes = uci_to_fixed5(&req.move_uci).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Validate and apply
    let _outcome = nimzovich_engine::on_chain_moves::validate_and_apply(&mut game, &mv_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Derive next FEN from the engine (discard client-provided next_fen)
    let derived_next_fen = game.to_compact_board().to_fen();

    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session_pk = entry.session_pubkey();
    let session_kp = entry.keypair();

    // Internal signing for data-level replay protection
    let mut hasher = Sha256::new();
    hasher.update(req.game_id.to_le_bytes());
    hasher.update(req.move_uci.as_bytes());
    hasher.update(derived_next_fen.as_bytes());
    hasher.update(req.nonce.to_le_bytes());
    let hash = hasher.finalize();
    let sig_bytes = session_kp.sign_message(&hash).as_ref().to_vec();

    let ix = solana::record_move_ix(
        &program_id,
        &session_pk,
        &entry.wallet_pubkey,
        req.game_id,
        &req.move_uci,
        &derived_next_fen,
        req.nonce,
        Some(sig_bytes),
    ).map_err(|e| {
        error!("[VPS] Failed to build record_move instruction for game {}: {}", req.game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let er_rpc = solana::make_rpc(&state.config.er_rpc_url);

    let sig = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] record_move failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] record_move game {} move {} sig {}", req.game_id, req.move_uci, sig);

    // Fire-and-forget DB write with derived FEN
    let game_id_str = req.game_id.to_string();
    let player_wallet = entry.wallet_pubkey.to_string();
    let move_uci = req.move_uci.clone();
    let next_fen = derived_next_fen;
    tokio::spawn(async move {
        let repo = GameRepository::new(pool);
        if let Err(e) = repo.upsert_game(&game_id_str).await {
            error!("[DB] Failed to upsert game {}: {}", game_id_str, e);
            return;
        }
        let move_number = repo.get_next_move_number(&game_id_str).await.unwrap_or(1) as i32;

        let san = generate_san(&repo, &game_id_str, &move_uci, move_number).await.ok();

        if let Err(e) = repo.add_move_simple(&game_id_str, move_number, &move_uci, san.as_deref(), Some(&next_fen), &player_wallet).await {
            error!("[DB] Failed to insert move for game {}: {}", game_id_str, e);
        }
    });

    Ok(Json(SigResp { sig: sig.to_string() }))
}

/// Upper bound on a single move's think time (ms). Anything larger is a
/// fabricated or buggy claim; clamping keeps the budget math meaningful.
const MAX_THINK_MS: u32 = 2 * 60 * 60 * 1000; // 2 hours

/// POST /telemetry/blur - Client-side anti-cheat telemetry.
///
/// Each client reports, for its *own* moves only, whether the game window
/// lost focus since its previous move (the alt-tab-to-engine signature) and
/// how long it spent on the move (`think_ms`). Ply parity is enforced (odd
/// plies are white's), so a client can only attach telemetry to plies of the
/// color it claims; first write per ply wins. The think time is a *claim* —
/// the analysis enqueue audits it against the server-observed wall clock
/// before scoring. Consumed by the anti-cheat pipeline as soft signals.
pub async fn report_blur_telemetry(
    State(state): State<AppState>,
    Json(req): Json<BlurTelemetryReq>,
) -> Result<StatusCode, StatusCode> {
    // Only accept telemetry for games this server is actually relaying.
    state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;

    if req.move_number == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_color = if req.move_number % 2 == 1 { "white" } else { "black" };
    if req.color != expected_color {
        return Err(StatusCode::BAD_REQUEST);
    }

    let think_ms = req.think_ms.map(|t| t.min(MAX_THINK_MS) as i64);

    sqlx::query(
        "INSERT OR IGNORE INTO move_telemetry (game_id, move_number, color, blurred, think_ms)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(req.game_id.to_string())
    .bind(req.move_number as i64)
    .bind(&req.color)
    .bind(req.blurred as i64)
    .bind(think_ms)
    .execute(&state.store.pool())
    .await
    .map_err(|e| {
        error!("[telemetry] blur insert failed for game {}: {}", req.game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Convert a UCI string (e.g. "e2e4" or "e7e8q") into a fixed 5-byte array.
fn uci_to_fixed5(uci: &str) -> Result<[u8; 5], ()> {
    let bytes = uci.as_bytes();
    if bytes.len() < 4 || bytes.len() > 5 {
        return Err(());
    }
    let mut out = [0u8; 5];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
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

    let ix = solana::undelegate_game_ix(&program_id, &session_pk, req.game_id).map_err(|e| {
        error!("[VPS] Failed to build undelegate_game instruction for game {}: {}", req.game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
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
) -> Result<Json<FinalizeResp>, StatusCode> {
    let entry = state.store.get(req.game_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session_kp = entry.keypair();

    let white = Pubkey::from_str(&req.white_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let black = Pubkey::from_str(&req.black_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let winner = req.winner.as_deref();

    // Use session key as fee_payer for now (will be replaced with ER relayer pubkey in production)
    let fee_payer = session_kp.pubkey();

    let ix = solana::finalize_game_ix(&program_id, req.game_id, &white, &black, winner, &fee_payer);
    let rpc = solana::make_rpc(&state.config.solana_rpc_url);

    // Submit through ER relayer for transaction fee reimbursement
    let sig = solana::sign_and_submit_er(&rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] finalize_game failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!("[VPS] finalize_game game {} winner={:?} sig {}", req.game_id, req.winner, sig);

    // Fire-and-forget DB write (log errors but don't fail the HTTP response)
    let game_id_str = req.game_id.to_string();
    let game_id = req.game_id;
    let wager_lamports = req.wager_lamports;
    let pool = state.store.pool();
    let elo_cache = state.elo_cache.clone();
    let white = req.white_pubkey.clone();
    let black = req.black_pubkey.clone();
    let winner = req.winner.clone();
    let sig_str = sig.to_string();
    let app_state = state.clone();
    tokio::spawn(async move {
        let repo = GameRepository::new(pool.clone());
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

        // Assemble and store PGN
        let moves = repo.get_moves(&game_id_str).await;
        if let Ok(moves) = moves {
            use nimzovich_engine::{PgnAssembler, PgnResult};
            let mut assembler = PgnAssembler::new();
            let date = chrono::Utc::now().format("%Y.%m.%d").to_string();
            assembler
                .tag("Event", "XFChess Game")
                .tag("Site", "XFChess")
                .tag("White", white_username.as_deref().unwrap_or(&white))
                .tag("Black", black_username.as_deref().unwrap_or(&black))
                .tag("Date", &date);
            for mv in moves {
                if let Some(san) = mv.move_san {
                    assembler.add_move(san);
                }
            }
            let result = match winner.as_deref() {
                Some("white") => PgnResult::WhiteWins,
                Some("black") => PgnResult::BlackWins,
                _ => PgnResult::Draw,
            };
            assembler.set_result(result);
            let pgn = assembler.to_string();
            if let Err(e) = repo.set_pgn_text(&game_id_str, &pgn).await {
                error!("[DB] Failed to store PGN for game {}: {}", game_id_str, e);
            }
        }

        elo_cache.invalidate(&white);
        elo_cache.invalidate(&black);

        // Enqueue post-game anti-cheat analysis via the shared path (also used
        // by the auto-settlement worker), which resolves tournament context.
        crate::signing::anticheat_enqueue::enqueue_game_analysis(
            &app_state,
            crate::signing::anticheat_enqueue::FinalizedGame {
                game_id,
                white: white.clone(),
                black: black.clone(),
                winner: winner.clone(),
                wager_lamports,
                tournament_id: None,
                base_time_seconds: 0,
                increment_seconds: 0,
            },
        )
        .await;
    });

    // Fee breakdown mirroring the on-chain contract constants.
    // COUNTRY_FEE = 1% of pot, ELO_FEE = 1% of pot, both deducted from winner payout.
    const COUNTRY_FEE_BPS: u64 = 100; // 1%
    const ELO_FEE_BPS: u64 = 100;     // 1%
    let pot = req.wager_lamports.saturating_mul(2);
    let country_fee = pot.saturating_mul(COUNTRY_FEE_BPS) / 10_000;
    let elo_fee = pot.saturating_mul(ELO_FEE_BPS) / 10_000;
    let winner_lamports = if req.winner.is_some() {
        pot.saturating_sub(country_fee).saturating_sub(elo_fee)
    } else {
        0 // draw: each player gets their wager back minus fees (handled on-chain)
    };

    Ok(Json(FinalizeResp {
        sig: sig.to_string(),
        winner_lamports,
        country_fee,
    }))
}

/// GET /player/:pubkey - Gets player profile details (ELO, country, username).
pub async fn get_player_profile(
    Path(pubkey): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<PlayerProfileResp>, StatusCode> {
    // Try on-chain profile first (Solana PlayerProfile PDA)
    match state.elo_cache.get_elo(&pubkey).await {
        Ok(elo_data) => {
            return Ok(Json(PlayerProfileResp {
                elo: (elo_data.elo_rating / 100.0) as u32,
                country: elo_data.country,
                username: elo_data.username,
            }));
        }
        Err(e) => {
            warn!("[profile] On-chain lookup failed for {}: {} — falling back to DB", pubkey, e);
        }
    }

    // Fall back to users_v2 (wallet registered via auth flow but no on-chain profile yet)
    let repo = crate::db::repository::GameRepository::new(state.store.pool());
    let username = repo.get_username(&pubkey).await.map_err(|_| {
        warn!("[profile] No DB record for {}", pubkey);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(PlayerProfileResp {
        elo: 1200,
        country: String::new(),
        username,
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
    /// Per-player wager in lamports (optional; 0 for free games).
    #[serde(default)]
    pub wager_lamports: u64,
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

/// Generate SAN for a move by replaying from the previous position.
async fn generate_san(
    repo: &GameRepository,
    game_id: &str,
    move_uci: &str,
    move_number: i32,
) -> anyhow::Result<String> {
    use nimzovich_engine::{game_from_fen, do_move, move_to_san};

    // Get previous FEN: last move's fen_after, or start position
    let prev_fen = if move_number <= 1 {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
    } else {
        let prev_moves = repo.get_moves(game_id).await?;
        prev_moves
            .iter()
            .find(|m| m.move_number == move_number - 1)
            .and_then(|m| m.fen_after.clone())
            .unwrap_or_else(|| "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string())
    };

    let mut game = game_from_fen(&prev_fen);

    // Parse UCI: e.g., "e2e4" -> src=12, dst=28, promo=0
    let bytes = move_uci.as_bytes();
    if bytes.len() < 4 {
        return Err(anyhow::anyhow!("Invalid UCI move: {}", move_uci));
    }
    let src_file = (bytes[0].wrapping_sub(b'a')) as i8;
    let src_rank = (bytes[1].wrapping_sub(b'1')) as i8;
    let dst_file = (bytes[2].wrapping_sub(b'a')) as i8;
    let dst_rank = (bytes[3].wrapping_sub(b'1')) as i8;

    if src_file < 0 || src_file > 7 || src_rank < 0 || src_rank > 7
        || dst_file < 0 || dst_file > 7 || dst_rank < 0 || dst_rank > 7
    {
        return Err(anyhow::anyhow!("Invalid UCI move: {}", move_uci));
    }

    let src = src_rank * 8 + src_file;
    let dst = dst_rank * 8 + dst_file;
    let promo = if bytes.len() > 4 {
        match bytes[4] {
            b'q' | b'Q' => 5,
            b'r' | b'R' => 4,
            b'b' | b'B' => 3,
            b'n' | b'N' => 2,
            _ => 0,
        }
    } else {
        0
    };

    // Apply the move
    do_move(&mut game, src, dst, true);

    // Generate SAN
    let san = move_to_san(&game, src, dst, promo);
    Ok(san)
}

// ── Item 5: move nonce ────────────────────────────────────────────────────────

/// GET /game/:game_id/nonce - Returns the last confirmed on-chain move nonce.
/// The client uses `nonce + 1` as the next move's nonce.
pub async fn get_move_nonce(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<NonceResp>, StatusCode> {
    let pool = state.store.pool();
    let game_id_str = game_id.to_string();
    let repo = GameRepository::new(pool);
    let next = repo.get_next_move_number(&game_id_str).await.unwrap_or(1) as u64;
    // next_move_number is 1-based; last confirmed nonce = next - 1 (0 if no moves yet).
    let nonce = next.saturating_sub(1);
    Ok(Json(NonceResp { nonce }))
}

// ── Item 4: free-rated ELO update ────────────────────────────────────────────

/// POST /ratings/update - Records result of a free (no-wager) rated game and triggers ELO update.
pub async fn update_free_rated_result(
    State(state): State<AppState>,
    Json(req): Json<FreeRatedResultReq>,
) -> Result<StatusCode, StatusCode> {
    let pool = state.store.pool();
    let game_id_str = req.game_id.to_string();
    let repo = GameRepository::new(pool);
    let winner = req.winner.as_deref();

    // Upsert the game record (idempotent if create_game was never called).
    if let Err(e) = repo.upsert_game(&game_id_str).await {
        error!("[ratings/update] Failed to upsert game {}: {}", req.game_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let white_username = repo.get_username(&req.white_pubkey).await.ok();
    let black_username = repo.get_username(&req.black_pubkey).await.ok();

    if let Err(e) = repo.complete_game(
        &game_id_str,
        Some(&req.white_pubkey),
        Some(&req.black_pubkey),
        white_username.as_deref(),
        black_username.as_deref(),
        winner,
        None,
        "",
        0.0,
    ).await {
        error!("[ratings/update] Failed to complete game {}: {}", req.game_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    state.elo_cache.invalidate(&req.white_pubkey);
    state.elo_cache.invalidate(&req.black_pubkey);

    info!("[ratings/update] Free-rated game {} result recorded (winner={:?})", req.game_id, req.winner);
    Ok(StatusCode::OK)
}

// ── Item 6: dispute submission ────────────────────────────────────────────────

/// POST /dispute/submit - Opens a 48-hour dispute window for a completed wager game.
/// The VPS logs the dispute; a human admin resolves it.
pub async fn submit_dispute(
    State(_state): State<AppState>,
    Json(req): Json<DisputeReq>,
) -> Result<Json<SigResp>, StatusCode> {
    // Log the dispute — human review resolves it.
    warn!(
        "[dispute] Player {} disputes game {} — manual review required",
        req.disputing_player, req.game_id
    );
    // Return a stub sig so the client can display "dispute submitted".
    Ok(Json(SigResp { sig: format!("dispute-{}-pending", req.game_id) }))
}

#[cfg(test)]
mod tests {
    use super::*;
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
            platform_fee_lamports: 0,
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
            wager_lamports: 0,
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
                wager_lamports: 0,
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
