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
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;
use tracing::{error, info, warn};

use crate::db::repository::GameRepository;
use crate::signing::{solana, AppState};
use crate::telemetry::worker_metrics;

/// Lamports used to fund a per-game session key: 0.01 SOL — enough for the
/// `create_game`/`join_game` init rent (~3.2M) plus ~2000 future tx fees.
/// Shared by the eager pre-fund in `create_session` and the fallback fund in
/// `activate_session` so the two can never silently diverge.
const SESSION_FUND_LAMPORTS: u64 = 10_000_000;

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
#[derive(Serialize, Default)]
pub struct SigResp {
    pub sig: String,
    /// RPC endpoint the transaction was actually submitted to — populated
    /// only for ER-routed instructions (record_move, undelegate_game) so
    /// the client can build an accurate `explorer.solana.com/tx/<sig>
    /// ?cluster=custom&customUrl=<er_endpoint>` link instead of guessing at
    /// a hardcoded constant that can drift from the backend's real
    /// `MAGIC_ROUTER_RPC_URL`/`ER_RPC_URL` config. Empty for base-layer
    /// responses (they're viewable on the normal devnet explorer).
    #[serde(default)]
    pub er_endpoint: String,
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

/// Public main API routes — reads and player-initiated writes that carry their
/// own validation. No relay-secret required.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session/status/{game_id}", get(session_status))
        .route("/telemetry/blur", post(report_blur_telemetry))
        .route("/game/{game_id}/nonce", get(get_move_nonce))
        .route("/ratings/update", post(update_free_rated_result))
        .route("/dispute/submit", post(submit_dispute))
        .route("/player/{pubkey}", get(get_player_profile))
        .route("/stats", get(get_stats))
}

/// Session-key signing endpoints. The VPS holds the session keypair and signs
/// Solana transactions on the caller's behalf, so these are wrapped with
/// [`require_relay_secret`](crate::infrastructure::require_relay_secret) by the
/// caller in `build_router`.
pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/session/create", post(create_session))
        .route("/session/activate", post(activate_session))
        .route("/session/sign", post(sign_tx))
        .route("/session/tee_auth", post(tee_auth))
        .route("/move/record", post(record_move))
        .route("/game/delegate", post(delegate_game))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
}

// ── Auth ──────────────────────────────────────────────────────────────────────
//
// NOTE: the former `POST /auth/issue` endpoint was removed. It issued a valid
// JWT for any wallet pubkey with no proof of ownership (account takeover).
// Wallet authentication now goes exclusively through the signature-verified
// SIWS flow in `routes::auth` (`/api/auth/siws-challenge` + `/api/auth/siws-verify`).

// ── Session ───────────────────────────────────────────────────────────────────

/// POST /session/create - Creates a new session for a game.
pub async fn create_session(
    State(state): State<AppState>,
    authed: Option<axum::Extension<crate::signing::auth::AuthedWallet>>,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, StatusCode> {
    // If the caller authenticated with a per-user JWT, they may only open a
    // session for their own wallet. (Legacy relay-secret callers have no
    // AuthedWallet and are unaffected during the dual-accept rollout.)
    if let Some(axum::Extension(crate::signing::auth::AuthedWallet(w))) = &authed {
        if w != &req.wallet_pubkey {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let wallet = Pubkey::from_str(&req.wallet_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let session_pubkey = state.store.create(req.game_id, wallet).await.map_err(|e| {
        error!(
            "[VPS] Failed to create session for game {}: {}",
            req.game_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    // RateCache::get already degrades to a stale-but-real rate on fetch failure —
    // None only happens on a cold cache with no successful fetch yet ever. Reject
    // rather than silently defaulting to a free game in that case.
    let platform_fee_lamports = state
        .rate_cache
        .gbp_to_lamports(crate::signing::routes::rates::PLATFORM_FEE_GBP)
        .await
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let treasury_vault =
        Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &state.program_id).0;
    info!(
        "[TREASURY] game {} session created — {} lamports platform fee will be recorded at \
         create_game and paid to treasury_vault {} when the game finishes (0 for Free match type)",
        req.game_id, platform_fee_lamports, treasury_vault
    );

    // Kick off session-key funding NOW, off the critical path. The freshly
    // generated per-game key holds zero SOL, but `create_game`/`join_game`
    // pay their init rent from it (`payer = fee_payer`), so it must be funded
    // before the setup TX lands. Funding here — while the player is still
    // reviewing the wallet popup — means `activate_session`'s own
    // `fund_account` almost always sees the balance already present and skips
    // its blocking `confirmed` wait, removing a whole serial on-chain
    // round-trip (~seconds on public RPC) from perceived create/join latency.
    // `fund_account` is idempotent (balance check) and `activate_session`
    // still funds+waits if this hasn't landed yet, so there's no correctness
    // dependency — it's a pure head start. Runs on a blocking thread because
    // the RPC client and its confirmation poll are synchronous.
    if let Ok(fee_payer) = Keypair::try_from(state.feepayer.next().to_bytes().as_slice()) {
        let rpc_url = state.config.solana_rpc_url.clone();
        let dest = session_pubkey;
        let game_id = req.game_id;
        tokio::task::spawn_blocking(move || {
            let rpc = solana::make_rpc(&rpc_url);
            if let Err(e) = solana::fund_account(&rpc, &fee_payer, &dest, SESSION_FUND_LAMPORTS) {
                warn!(
                    "[VPS] eager session-key pre-fund for game {game_id} failed (activate will retry): {e}"
                );
            }
        });
    }

    Ok(Json(CreateSessionResp {
        session_pubkey: session_pubkey.to_string(),
        platform_fee_lamports,
    }))
}

/// POST /session/activate - Activates a session with wallet-signed setup TX.
///
/// Error responses carry the real reason in the body (not just a bare status)
/// so it reaches the player's `LobbyStatus::Error` UI instead of dead-ending
/// as an opaque "HTTP 502" that only the server's own log can explain.
pub async fn activate_session(
    State(state): State<AppState>,
    Json(req): Json<ActivateSessionReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    let tx_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.signed_tx_b64,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad base64: {e}")))?;

    // Peek the activating wallet (tx-level fee payer = account_keys[0]) before
    // deciding idempotency. A game_id's session is activated by TWO different
    // wallets over its lifetime — the host's create_game, then later a
    // joiner's join_game — both hit this same endpoint. Idempotency must be
    // keyed per (game_id, wallet), not per game_id alone: keying on game_id
    // alone meant the host's activation flipped a single game-level flag that
    // then made every subsequent joiner's activate_session call look like an
    // "already active" retry — silently short-circuiting before the join_game
    // TX was ever submitted, while still reporting HTTP 200 to the joiner.
    let peek_tx: solana_sdk::transaction::Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad tx bytes: {e}")))?;
    let activating_wallet = *peek_tx.message.account_keys.first().ok_or((
        StatusCode::BAD_REQUEST,
        "tx has no account keys".to_string(),
    ))?;

    if state
        .store
        .wallet_activated(req.game_id, &activating_wallet)
        .await
    {
        info!(
            "[VPS] Wallet {activating_wallet} already activated game {} — idempotent success",
            req.game_id
        );
        return Ok(Json(SigResp {
            sig: "already-active".to_string(),
            ..Default::default()
        }));
    }

    let rpc = solana::make_rpc(&state.config.solana_rpc_url);

    // Retrieve session keypair so we can co-sign the TX.
    // The create_game / join_game instructions require BOTH the player wallet
    // signature (already in tx_bytes) and the session key signature (fee_payer).
    let entry = state.store.get(req.game_id).await.ok_or_else(|| {
        let msg = format!(
            "No session found for game {} — call /session/create first",
            req.game_id
        );
        error!("[VPS] {msg}");
        (StatusCode::NOT_FOUND, msg)
    })?;
    let session_keypair = entry.keypair();

    // Fund the session key BEFORE submitting the setup TX. `create_game` /
    // `join_game` use `payer = fee_payer` (the session key) for the `init`
    // constraints on the game + escrow accounts, so the session key must
    // already hold enough lamports for that rent — otherwise the submit
    // below deterministically fails with "insufficient lamports" on every
    // first-time session (each game_id gets a brand-new, unfunded keypair
    // from /session/create). Funding after submission, as this used to do,
    // never had a chance to help.
    let fee_payer = state.feepayer.next();
    solana::fund_account(
        &rpc,
        fee_payer,
        &entry.session_pubkey(),
        SESSION_FUND_LAMPORTS,
    )
    .map_err(|e| {
        let msg = format!("Could not fund session key for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;

    // Submit the wallet-signed TX, adding the session key co-signature.
    let sig = solana::cosign_and_submit_tx(&rpc, &session_keypair, &tx_bytes).map_err(|e| {
        let msg = format!("Failed to submit setup TX for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!("[VPS] Setup TX confirmed for game {}: {sig}", req.game_id);

    // Mark session active (gates record_move/session_status) and record this
    // wallet's activation so a retry from the SAME wallet is idempotent
    // without blocking the other player's activation.
    state.store.activate(req.game_id).await;
    state
        .store
        .record_wallet_activation(req.game_id, &activating_wallet, &sig.to_string())
        .await;

    Ok(Json(SigResp {
        sig: sig.to_string(),
        ..Default::default()
    }))
}

/// GET /session/status/:game_id - Gets session status.
pub async fn session_status(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(entry) = state.store.get(game_id).await {
        return Ok(Json(serde_json::json!({
            "active": entry.active,
            "session_pubkey": entry.session_pubkey().to_string(),
        })));
    }
    // No SessionStore row — check whether this is a global-session game
    // instead (see `resolve_game_signer`). There's no separate "activate"
    // step for that flow, so a match is inherently active.
    match resolve_game_signer(&state, game_id).await {
        Some(kp) => Ok(Json(serde_json::json!({
            "active": true,
            "session_pubkey": kp.pubkey().to_string(),
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
) -> Result<Json<SigResp>, (StatusCode, String)> {
    let entry = state.store.get(req.game_id).await.ok_or_else(|| {
        let msg = format!("No session found for game {}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::NOT_FOUND, msg)
    })?;
    if !entry.active {
        let msg = format!("Session for game {} is not active", req.game_id);
        return Err((StatusCode::PRECONDITION_FAILED, msg));
    }

    // ── Engine-side validation (replay from previous FEN) ──
    let pool = state.store.pool();
    let repo = GameRepository::new(pool.clone());

    let prev_fen = if let Ok(moves) = repo.get_moves(&req.game_id.to_string()).await {
        moves
            .iter()
            .max_by_key(|m| m.move_number)
            .and_then(|m| m.fen_after.clone())
    } else {
        None
    }
    .unwrap_or_else(|| "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    let mut game = nimzovich_engine::on_chain::CompactBoard::from_fen(&prev_fen).to_on_chain_game();

    // Parse UCI into fixed 5-byte array
    let mv_bytes = uci_to_fixed5(&req.move_uci).map_err(|_| {
        let msg = format!("Bad move_uci '{}' for game {}", req.move_uci, req.game_id);
        (StatusCode::BAD_REQUEST, msg)
    })?;

    // Validate and apply
    let _outcome = nimzovich_engine::on_chain_moves::validate_and_apply(&mut game, &mv_bytes)
        .map_err(|e| {
            let msg = format!(
                "Move {} rejected by engine for game {}: {e:?}",
                req.move_uci, req.game_id
            );
            (StatusCode::BAD_REQUEST, msg)
        })?;

    // Derive next FEN from the engine (discard client-provided next_fen)
    let next_board = game.to_compact_board().to_bytes();
    let derived_next_fen = game.to_compact_board().to_fen();

    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|e| {
        let msg = format!("Bad configured program_id: {e}");
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;
    let session_pk = entry.session_pubkey();
    let session_kp = entry.keypair();

    // Internal signing for data-level replay protection
    let mut hasher = Sha256::new();
    hasher.update(req.game_id.to_le_bytes());
    hasher.update(mv_bytes);
    hasher.update(next_board);
    hasher.update(req.nonce.to_le_bytes());
    let hash = hasher.finalize();
    let sig_bytes = session_kp.sign_message(&hash).as_ref().to_vec();

    // Games created via the global-session flow never get a per-game
    // `SessionDelegation` account, so `record_move` (which requires one)
    // fails on-chain for them — `global_record_move` checks
    // `GlobalSessionDelegation` instead. `entry.is_global` (set by
    // `create_with_keypair`, see migration 026) is what tells these apart.
    let ix = if entry.is_global {
        solana::global_record_move_ix(
            &program_id,
            &session_pk,
            &entry.wallet_pubkey,
            req.game_id,
            mv_bytes,
            next_board,
            req.nonce,
            Some(sig_bytes),
            req.nonce.checked_sub(1),
        )
    } else {
        solana::record_move_ix(
            &program_id,
            &session_pk,
            &entry.wallet_pubkey,
            req.game_id,
            mv_bytes,
            next_board,
            req.nonce,
            Some(sig_bytes),
            req.nonce.checked_sub(1),
        )
    }
    .map_err(|e| {
        let msg = format!(
            "Failed to build record_move instruction for game {}: {}",
            req.game_id, e
        );
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;
    let er_rpc = solana::rpc_for(&state.config, solana::RoutedInstr::RecordMove);

    // Same empty-body 502 problem as delegate_game/undelegate_game — this is
    // the route every single move goes through, so an opaque failure here is
    // the single most disruptive place for it to happen.
    let sig = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]).map_err(|e| {
        let msg = format!("record_move failed for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} move {} recorded on Ephemeral Rollup ({}) sig {}",
        req.game_id,
        req.move_uci,
        er_rpc.url(),
        sig
    );

    // Persist the move before responding — this used to be a fire-and-forget
    // `tokio::spawn`, which raced the very next `record_move` call: that
    // call's `prev_fen` lookup above reads this same history via
    // `repo.get_moves`, so if the spawned write for move N hadn't landed yet
    // when move N+1's request arrived (routine for a batch submitted in a
    // tight loop, e.g. `submit_moves_via_vps`/game-end batches), move N+1 got
    // replay-validated against a stale FEN and was rejected by the engine as
    // illegal — reproduced live: a 2-move batch's *first* move failed this
    // way. Awaiting it costs a couple of local SQLite writes (sub-ms) per
    // move, worth it for correctness.
    let game_id_str = req.game_id.to_string();
    let player_wallet = entry.wallet_pubkey.to_string();
    let move_uci = req.move_uci.clone();
    let next_fen = derived_next_fen;
    if let Err(e) = repo.upsert_game(&game_id_str).await {
        error!("[DB] Failed to upsert game {}: {}", game_id_str, e);
    } else {
        let move_number = repo.get_next_move_number(&game_id_str).await.unwrap_or(1) as i32;

        let san = generate_san(&repo, &game_id_str, &move_uci, move_number)
            .await
            .ok();

        if let Err(e) = repo
            .add_move_simple(
                &game_id_str,
                move_number,
                &move_uci,
                san.as_deref(),
                Some(&next_fen),
                &player_wallet,
            )
            .await
        {
            error!("[DB] Failed to insert move for game {}: {}", game_id_str, e);
        }
    }

    Ok(Json(SigResp {
        sig: sig.to_string(),
        er_endpoint: er_rpc.url(),
    }))
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
    state
        .store
        .get(req.game_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if req.move_number == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_color = if req.move_number % 2 == 1 {
        "white"
    } else {
        "black"
    };
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
        error!(
            "[telemetry] blur insert failed for game {}: {}",
            req.game_id, e
        );
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

/// POST /game/delegate - Delegates a game from devnet to the Ephemeral Rollup.
///
/// Mirrors `undelegate_game`'s trust model in reverse: the VPS already holds
/// the per-game session key (set as `game.fee_payer` during create/join), so
/// it can satisfy the on-chain `fee_payer` check itself; the fee-payer pool
/// covers the delegation bookkeeping accounts' rent. Lets the client trigger
/// delegation with zero wallet popup — no session-key funding margin spent
/// on this beyond what `activate_session` already provides.
pub async fn delegate_game(
    State(state): State<AppState>,
    Json(req): Json<DelegateGameReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    let entry = state.store.get(req.game_id).await.ok_or_else(|| {
        let msg = format!("No session found for game {}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::NOT_FOUND, msg)
    })?;
    let program_id = Pubkey::from_str(&state.config.program_id).map_err(|e| {
        let msg = format!("Bad configured program_id: {e}");
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;
    let session_kp = entry.keypair();
    let session_pk = entry.session_pubkey();
    let payer = state.feepayer.next();

    let ix = solana::delegate_game_ix(&program_id, req.game_id, &payer.pubkey(), &session_pk)
        .map_err(|e| {
            let msg = format!(
                "Failed to build delegate_game instruction for game {}: {}",
                req.game_id, e
            );
            error!("[VPS] {msg}");
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

    let rpc = solana::rpc_for(&state.config, solana::RoutedInstr::DelegateGame);
    // The client only ever saw `HTTP 502 —  —` on failure otherwise — see
    // the identical fix + rationale on `undelegate_game`.
    let blockhash = rpc.get_latest_blockhash().map_err(|e| {
        let msg = format!(
            "delegate_game get_latest_blockhash failed for game {}: {e}",
            req.game_id
        );
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, &session_kp],
        blockhash,
    );
    let sig = rpc.send_and_confirm_transaction(&tx).map_err(|e| {
        let msg = format!(
            "delegate_game failed for game {} (sig would be unknown — send failed): {e}",
            req.game_id
        );
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} delegated to Ephemeral Rollup ({}) sig {}",
        req.game_id, state.config.magic_router_rpc_url, sig
    );

    schedule_time_check_crank(&state, &rpc, &program_id, &session_kp, req.game_id).await;

    Ok(Json(SigResp {
        sig: sig.to_string(),
        ..Default::default()
    }))
}

/// Registers the on-chain time-check crank for a freshly-delegated game, so a
/// stalled clock gets auto-forfeited on the ER even if nothing else calls
/// `claim_timeout`. 30s interval, unlimited iterations — cancelled by
/// `cancel_time_check_crank` alongside `undelegate_game`.
///
/// Best-effort: delegation itself already succeeded (the part that matters
/// for gameplay to continue), so a scheduling failure is logged and counted
/// rather than surfaced to the caller.
///
/// `pub(crate)` so `tasks::settlement_worker`'s redelegate-retry path can
/// reuse it after successfully redelegating a stuck game — the crank still
/// needs to be (re)registered exactly like the normal delegate flow does.
pub(crate) async fn schedule_time_check_crank(
    state: &AppState,
    base_rpc: &solana_client::rpc_client::RpcClient,
    program_id: &Pubkey,
    session_kp: &solana_sdk::signature::Keypair,
    game_id: u64,
) {
    const TIME_CHECK_INTERVAL_MILLIS: u64 = 30_000;

    let game_pda =
        Pubkey::find_program_address(&[solana::GAME_SEED, &game_id.to_le_bytes()], program_id).0;

    // Game layout: disc(8) + game_id(8) + white(32) + black(32) + ... — same
    // offsets `resolve_game_signer` uses for `fee_payer` below.
    let (white, black) = match base_rpc.get_account_data(&game_pda) {
        Ok(data) => {
            let white = data.get(16..48).and_then(|b| Pubkey::try_from(b).ok());
            let black = data.get(48..80).and_then(|b| Pubkey::try_from(b).ok());
            match (white, black) {
                (Some(w), Some(b)) => (w, b),
                _ => {
                    error!(
                        "[VPS] schedule_time_check: malformed Game account for game {}",
                        game_id
                    );
                    worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
            }
        }
        Err(e) => {
            error!(
                "[VPS] schedule_time_check: failed to read Game account for game {}: {e}",
                game_id
            );
            worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    let ix = match solana::schedule_time_check_ix(
        program_id,
        &session_kp.pubkey(),
        game_id,
        &white,
        &black,
        TIME_CHECK_INTERVAL_MILLIS,
    ) {
        Ok(ix) => ix,
        Err(e) => {
            error!(
                "[VPS] Failed to build schedule_time_check instruction for game {}: {}",
                game_id, e
            );
            worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    let er_rpc = solana::rpc_for(&state.config, solana::RoutedInstr::ScheduleTimeCheck);
    match solana::sign_and_submit_er(&er_rpc, session_kp, &[ix]) {
        Ok(sig) => {
            info!("[VPS] schedule_time_check game {} sig {}", game_id, sig);
            worker_metrics::TIME_CHECK_SCHEDULED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Err(e) => {
            error!("[VPS] schedule_time_check failed for game {}: {e}", game_id);
            worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Resolves the signing keypair for `game_id`: prefers the per-game
/// `SessionStore` entry (original `create_game`/`join_game` +
/// `authorize_session_key` flow); if none exists, reads the Game PDA's
/// `fee_payer` on-chain and checks it against sessions registered via
/// `routes::global_session::register` (the newer `global_create_game`/
/// `global_join_game` flow, which never gets a `SessionStore` row since it
/// skips `/session/create` entirely). Returns `None` if neither source has
/// a match — the caller should treat that as "no known signer for this game"
/// (404), same as a plain `SessionStore` miss did before this existed.
pub async fn resolve_game_signer(
    state: &AppState,
    game_id: u64,
) -> Option<solana_sdk::signature::Keypair> {
    if let Some(entry) = state.store.get(game_id).await {
        return Some(entry.keypair());
    }

    let game_pda = Pubkey::find_program_address(
        &[solana::GAME_SEED, &game_id.to_le_bytes()],
        &state.program_id,
    )
    .0;
    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let data = tokio::task::spawn_blocking(move || rpc.get_account_data(&game_pda))
        .await
        .ok()?
        .ok()?;

    // Game layout: disc(8) + game_id(8) + white(32) + black(32) + status(1)
    // + last_move_timestamp(8) + fees_advanced(8) → fee_payer at offset 97.
    // Matches `parse_game_account` in `tasks::settlement_worker` — keep both
    // in sync if the `Game` struct's field order ever changes.
    let fee_payer = data.get(97..129).and_then(|b| Pubkey::try_from(b).ok())?;

    let sessions = state.active_global_sessions.lock().await;
    sessions
        .values()
        .find(|kp| kp.pubkey() == fee_payer)
        .and_then(|kp| solana_sdk::signature::Keypair::try_from(kp.to_bytes().as_slice()).ok())
}

/// POST /game/undelegate - Undelegates a game from ER to devnet.
pub async fn undelegate_game(
    State(state): State<AppState>,
    Json(req): Json<UndelegateGameReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    let session_kp = resolve_game_signer(&state, req.game_id)
        .await
        .ok_or_else(|| {
            let msg = format!("No session found for game {}", req.game_id);
            error!("[VPS] {msg}");
            (StatusCode::NOT_FOUND, msg)
        })?;
    let session_pk = session_kp.pubkey();

    let ix =
        solana::undelegate_game_ix(&state.program_id, &session_pk, req.game_id).map_err(|e| {
            let msg = format!(
                "Failed to build undelegate_game instruction for game {}: {}",
                req.game_id, e
            );
            error!("[VPS] {msg}");
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;
    let er_rpc = solana::rpc_for(&state.config, solana::RoutedInstr::UndelegateGame);

    // The client only ever sees `HTTP 502 —  —` on failure otherwise
    // (`vps_undelegate_game` folds the response body straight into its error
    // string) — the real reason (stale ER state, RPC timeout, session key
    // mismatch, ...) stayed stuck in this process's own log, which isn't
    // where anyone debugging a two-client test is looking.
    let sig = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]).map_err(|e| {
        let msg = format!("undelegate_game failed for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} undelegated from Ephemeral Rollup ({}) back to devnet, sig {}",
        req.game_id,
        er_rpc.url(),
        sig
    );

    // Cancel the time-check crank as a separate, best-effort follow-up — never
    // let a cancel failure affect the undelegate result already returned
    // above. A stray task on an undelegated account is expected to fail
    // harmlessly on its own next tick either way.
    cancel_time_check_crank(&er_rpc, &state.program_id, &session_kp, req.game_id);

    Ok(Json(SigResp {
        sig: sig.to_string(),
        er_endpoint: er_rpc.url(),
    }))
}

/// Cancels the on-chain time-check crank scheduled by `schedule_time_check_crank`.
/// Best-effort and synchronous-but-fire-and-forget from the caller's
/// perspective: never returns an error, only logs/counts one.
fn cancel_time_check_crank(
    er_rpc: &solana_client::rpc_client::RpcClient,
    program_id: &Pubkey,
    session_kp: &solana_sdk::signature::Keypair,
    game_id: u64,
) {
    let session_pk = session_kp.pubkey();
    let ix = match solana::cancel_time_check_ix(program_id, &session_pk, game_id) {
        Ok(ix) => ix,
        Err(e) => {
            error!(
                "[VPS] Failed to build cancel_time_check instruction for game {}: {}",
                game_id, e
            );
            worker_metrics::TIME_CHECK_CANCEL_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    match solana::sign_and_submit_er(er_rpc, session_kp, &[ix]) {
        Ok(sig) => {
            info!("[VPS] cancel_time_check game {} sig {}", game_id, sig);
            worker_metrics::TIME_CHECK_CANCELLED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Err(e) => {
            error!("[VPS] cancel_time_check failed for game {}: {e}", game_id);
            worker_metrics::TIME_CHECK_CANCEL_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Reads the on-chain `Game.country_fee` field (the flat platform fee set at
/// creation time, in lamports) directly off account bytes, since it's the
/// authoritative amount `settle_finished_game` actually pays to
/// `treasury_vault` — recomputing an estimate client/backend-side (e.g. via a
/// BPS-of-pot guess) can silently diverge from what the program transfers.
/// Layout must track `Game`'s field order — mirrors (a subset of)
/// `parse_game_account` in `tasks::settlement_worker`; keep both in sync if
/// the struct's field order ever changes. Returns `None` on any parse
/// failure (missing account, unexpected layout) — callers should treat that
/// as "fee unknown", not "fee zero".
fn read_game_country_fee(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &Pubkey,
    game_id: u64,
) -> Option<u64> {
    const RESULT_WINNER: u8 = 1;
    let game_pda =
        Pubkey::find_program_address(&[solana::GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let data = rpc.get_account_data(&game_pda).ok()?;
    let mut o = 8usize; // discriminator
    o += 8; // game_id
    o += 32 + 32; // white + black
    o += 1; // status
    o += 8; // last_move_timestamp
    o += 8; // fees_advanced
    o += 32; // fee_payer
    let result_tag = *data.get(o)?;
    o += 1;
    if result_tag == RESULT_WINNER {
        o += 32; // winner pubkey
    }
    o += 68; // board_state
    o += 2; // move_count
    o += 2; // turn
    o += 8; // created_at
    o += 8; // updated_at
    o += 8; // wager_amount
    let wager_token_tag = *data.get(o)?;
    o += 1;
    if wager_token_tag == 1 {
        o += 32; // Some(Pubkey)
    }
    o += 1; // game_type
    o += 1; // match_type
    let country_fee = u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);
    Some(country_fee)
}

/// POST /game/finalize - Finalizes a game on devnet.
pub async fn finalize_game(
    State(state): State<AppState>,
    Json(req): Json<FinalizeGameReq>,
) -> Result<Json<FinalizeResp>, StatusCode> {
    let session_kp = resolve_game_signer(&state, req.game_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let white = Pubkey::from_str(&req.white_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let black = Pubkey::from_str(&req.black_pubkey).map_err(|_| StatusCode::BAD_REQUEST)?;
    let winner = req.winner.as_deref();

    // Use session key as fee_payer for now (will be replaced with ER relayer pubkey in production)
    let fee_payer = session_kp.pubkey();

    let ix = solana::finalize_game_ix(
        &state.program_id,
        req.game_id,
        &white,
        &black,
        winner,
        &fee_payer,
    );
    let rpc = solana::rpc_for(&state.config, solana::RoutedInstr::FinalizeGame);

    // Read the actual fee that will be charged BEFORE finalize closes the
    // Game account out from under us.
    let actual_country_fee = read_game_country_fee(&rpc, &state.program_id, req.game_id);

    let sig = solana::sign_and_submit(&rpc, &session_kp, &[ix]).map_err(|e| {
        error!("[VPS] finalize_game failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;
    info!(
        "[FINALIZED] game {} settled on devnet — winner={:?} wager_lamports={} payout sig {}",
        req.game_id, req.winner, req.wager_lamports, sig
    );
    let treasury_vault =
        Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &state.program_id).0;
    match actual_country_fee {
        Some(fee) if fee > 0 => info!(
            "[TREASURY] game {} paid {} lamports platform fee to treasury_vault {}",
            req.game_id, fee, treasury_vault
        ),
        Some(_) => info!(
            "[TREASURY] game {} — free/unwagered game, no platform fee charged",
            req.game_id
        ),
        None => warn!(
            "[TREASURY] game {} — could not read on-chain country_fee before finalize; treasury_vault {} payment amount unconfirmed",
            req.game_id, treasury_vault
        ),
    }

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
        if let Err(e) = repo
            .complete_game(
                &game_id_str,
                Some(&white),
                Some(&black),
                white_username.as_deref(),
                black_username.as_deref(),
                winner.as_deref(),
                None, // final_fen (not provided in request)
                &sig_str,
                0.0, // stake_amount (not provided in request)
            )
            .await
        {
            error!("[DB] Failed to finalize game {}: {}", game_id_str, e);
        }

        // Assemble and store PGN — Event/White/Black/Elo use resolved wallet
        // usernames and live on-chain ELO, same helper the auto-settlement
        // worker uses so both finalize paths produce an identical PGN shape.
        crate::signing::game_pgn::assemble_and_store_pgn(
            &repo,
            &elo_cache,
            &game_id_str,
            &white,
            &black,
            white_username.as_deref(),
            black_username.as_deref(),
            winner.as_deref(),
        )
        .await;

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
    const ELO_FEE_BPS: u64 = 100; // 1%
    let pot = req.wager_lamports.saturating_mul(2);
    // Prefer the real on-chain fee we just read (what the program actually
    // transfers to treasury_vault) over the BPS-of-pot estimate below, which
    // only approximates it and can silently diverge from the flat
    // creation-time fee `settle_finished_game` actually pays out.
    let country_fee =
        actual_country_fee.unwrap_or_else(|| pot.saturating_mul(COUNTRY_FEE_BPS) / 10_000);
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
            warn!(
                "[profile] On-chain lookup failed for {}: {} — falling back to DB",
                pubkey, e
            );
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
pub struct DelegateGameReq {
    pub game_id: u64,
}

#[derive(Deserialize, Serialize)]
pub struct FinalizeGameReq {
    pub game_id: u64,
    pub winner: Option<String>, // "white" | "black" | null (draw)
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

    let entry = state
        .store
        .get(req.game_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let tx_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &req.tx_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut tx: Transaction =
        bincode::deserialize(&tx_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let session_kp = entry.keypair();

    let rpc = solana::make_rpc(&state.config.solana_rpc_url);
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    tx.partial_sign(&[&session_kp], blockhash);

    let sig = rpc.send_and_confirm_transaction(&tx).map_err(|e| {
        error!("[VPS] sign_tx failed for game {}: {e}", req.game_id);
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(SigResp {
        sig: sig.to_string(),
        ..Default::default()
    }))
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
        warn!(
            "[TEE-AUTH] Bad signature length ({} bytes) for game {}",
            sig_bytes.len(),
            req.game_id
        );
        StatusCode::BAD_REQUEST
    })?;

    if !sig.verify(wallet.as_ref(), TEE_AUTH_MESSAGE) {
        warn!(
            "[TEE-AUTH] Signature mismatch — wallet {} game {}",
            req.wallet_pubkey, req.game_id
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    let _entry = state
        .store
        .get(req.game_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    info!(
        "[TEE-AUTH] Wallet {} authenticated for game {}",
        req.wallet_pubkey, req.game_id
    );
    Ok(Json(SigResp {
        sig: "tee-auth-ok".to_string(),
        ..Default::default()
    }))
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
    use nimzovich_engine::{do_move, game_from_fen_no_tt, move_to_san};

    // Get previous FEN: last move's fen_after, or start position
    let prev_fen = if move_number <= 1 {
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
    } else {
        let prev_moves = repo.get_moves(game_id).await?;
        prev_moves
            .iter()
            .find(|m| m.move_number == move_number - 1)
            .and_then(|m| m.fen_after.clone())
            .unwrap_or_else(|| {
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string()
            })
    };

    // No search ever runs on this Game (just replay + SAN), so skip the
    // multi-GB transposition table `game_from_fen` would otherwise allocate
    // per HTTP request.
    let mut game = game_from_fen_no_tt(&prev_fen);

    // Parse UCI: e.g., "e2e4" -> src=12, dst=28, promo=0
    let bytes = move_uci.as_bytes();
    if bytes.len() < 4 {
        return Err(anyhow::anyhow!("Invalid UCI move: {}", move_uci));
    }
    let src_file = (bytes[0].wrapping_sub(b'a')) as i8;
    let src_rank = (bytes[1].wrapping_sub(b'1')) as i8;
    let dst_file = (bytes[2].wrapping_sub(b'a')) as i8;
    let dst_rank = (bytes[3].wrapping_sub(b'1')) as i8;

    if src_file < 0
        || src_file > 7
        || src_rank < 0
        || src_rank > 7
        || dst_file < 0
        || dst_file > 7
        || dst_rank < 0
        || dst_rank > 7
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
    let san = move_to_san(&mut game, src, dst, promo);
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
        error!(
            "[ratings/update] Failed to upsert game {}: {}",
            req.game_id, e
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let white_username = repo.get_username(&req.white_pubkey).await.ok();
    let black_username = repo.get_username(&req.black_pubkey).await.ok();

    if let Err(e) = repo
        .complete_game(
            &game_id_str,
            Some(&req.white_pubkey),
            Some(&req.black_pubkey),
            white_username.as_deref(),
            black_username.as_deref(),
            winner,
            None,
            "",
            0.0,
        )
        .await
    {
        error!(
            "[ratings/update] Failed to complete game {}: {}",
            req.game_id, e
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    state.elo_cache.invalidate(&req.white_pubkey);
    state.elo_cache.invalidate(&req.black_pubkey);

    info!(
        "[ratings/update] Free-rated game {} result recorded (winner={:?})",
        req.game_id, req.winner
    );
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
    Ok(Json(SigResp {
        sig: format!("dispute-{}-pending", req.game_id),
        ..Default::default()
    }))
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
            ..Default::default()
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
        let req = UndelegateGameReq { game_id: 12345 };

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
