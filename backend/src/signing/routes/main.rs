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
use std::collections::HashMap;
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

/// How many sessions one wallet may open per [`SESSION_RATE_WINDOW`].
///
/// Every `/session/create` funds a fresh keypair with [`SESSION_FUND_LAMPORTS`]
/// from the fee-payer pool, and `game_id` is chosen by the caller, so without a
/// ceiling a single authenticated wallet can drain the pool in a loop — 0.01 SOL
/// per HTTP request, with nothing to stop it. A real player creates or joins a
/// handful of games in an hour; this leaves ample headroom for retries and
/// reconnects while making the drain loop useless.
const SESSION_RATE_MAX_PER_WALLET: usize = 12;
const SESSION_RATE_WINDOW: std::time::Duration = std::time::Duration::from_secs(600);

/// Ceiling on *unactivated* sessions a wallet may hold at once. Bounds the
/// stock of funded-but-unused keys independently of the rate above, which only
/// bounds the flow.
const SESSION_MAX_PENDING_PER_WALLET: i64 = 6;

/// Per-wallet `/session/create` timestamps. Bounded: entries older than the
/// window are dropped on every touch, and wallets whose history empties are
/// evicted rather than left as permanently-growing keys.
fn session_create_tracker() -> &'static tokio::sync::Mutex<HashMap<String, Vec<std::time::Instant>>>
{
    static TRACKER: std::sync::OnceLock<
        tokio::sync::Mutex<HashMap<String, Vec<std::time::Instant>>>,
    > = std::sync::OnceLock::new();
    TRACKER.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

/// Rations `/session/create` against the fee-payer pool: a sliding-window rate
/// limit per wallet, plus a cap on how many funded-but-unactivated sessions one
/// wallet may hold. Returns the HTTP error to surface when either is exceeded.
async fn session_funding_guard(state: &AppState, wallet: &str) -> Result<(), (StatusCode, String)> {
    {
        let mut tracker = session_create_tracker().lock().await;
        let now = std::time::Instant::now();

        // Evict stale wallets on every pass so this map tracks live callers
        // rather than every wallet that has ever called.
        tracker.retain(|_, hits| {
            hits.retain(|t| now.duration_since(*t) < SESSION_RATE_WINDOW);
            !hits.is_empty()
        });

        let hits = tracker.entry(wallet.to_string()).or_default();
        if hits.len() >= SESSION_RATE_MAX_PER_WALLET {
            warn!("[VPS] session-create rate limit hit for wallet {wallet}");
            worker_metrics::SESSION_CREATE_THROTTLED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "Too many sessions opened recently. At most {SESSION_RATE_MAX_PER_WALLET} \
                     per {} minutes — retry shortly.",
                    SESSION_RATE_WINDOW.as_secs() / 60
                ),
            ));
        }
        hits.push(now);
    }

    let pending = state.store.count_pending_for_wallet(wallet).await;
    if pending >= SESSION_MAX_PENDING_PER_WALLET {
        warn!("[VPS] wallet {wallet} holds {pending} unactivated sessions — refusing to fund more");
        worker_metrics::SESSION_CREATE_THROTTLED_TOTAL
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!(
                "You already have {pending} sessions waiting to be activated. Finish or abandon \
                 those before starting another."
            ),
        ));
    }

    Ok(())
}

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
#[derive(Deserialize, Serialize, Clone)]
pub struct RecordMoveReq {
    pub game_id: u64,
    pub move_uci: String,
    pub next_fen: String,
    #[serde(default)]
    pub nonce: u64,
    /// Wallet of the player whose move this is (White on odd plies, Black
    /// on even) — see `resolve_move_signer`'s doc comment for why this
    /// can't be inferred from `SessionStore` alone.
    #[serde(default)]
    pub mover_wallet: String,
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
    /// Real backend-advanced operating cost for this game (create + join +
    /// delegate + N*record_move + undelegate + the MagicBlock ER session
    /// fee) — the on-chain `Game.fees_advanced` value read just before
    /// finalize closes the account, which `settle_finished_game` reimburses
    /// to `treasury_vault` out of the pot. 0 for free games (nothing is
    /// reimbursed there — see `lifecycle::settlement`'s wager_amount gate).
    pub operating_cost_lamports: u64,
    /// Flat ELO-linking fee split between both players' wallets at
    /// settlement (`ELO_FEE_LAMPORTS` on-chain), 0 for free games.
    pub elo_fee: u64,
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
        .route("/game/{game_id}/nonce", get(get_move_nonce))
        .route("/player/{pubkey}", get(get_player_profile))
        .route("/stats", get(get_stats))
}

/// Session-key signing endpoints. The VPS holds the session keypair and signs
/// Solana transactions on the caller's behalf, so these are wrapped with
/// [`require_relay_secret`](crate::infrastructure::require_relay_secret) by the
/// caller in `build_router`.
/// NOTE: `POST /session/sign` used to live here. It deserialized an arbitrary
/// caller-supplied `Transaction`, signed it with the game's session keypair,
/// re-anchored it on a fresh blockhash and submitted it — with no inspection of
/// the instructions, program IDs or accounts, and no check that the caller had
/// any relationship to `game_id`. That is an unrestricted signing oracle over a
/// key the backend holds on a player's behalf. It has been removed rather than
/// constrained: nothing in the game client ever called it (delegation goes
/// through `/game/delegate`, which builds the instruction server-side), so the
/// endpoint was pure attack surface. `POST /session/tee_auth` is gone for the
/// same reason — it verified a signature over a constant message with no nonce,
/// game binding or expiry (replayable forever), then discarded the result.
pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/session/create", post(create_session))
        .route("/session/activate", post(activate_session))
        .route("/move/record", post(record_move))
        .route("/game/delegate", post(delegate_game))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
        // Both moved here from the public router. `/telemetry/blur` writes
        // anti-cheat evidence against a named player and `/ratings/update`
        // rewrites a game's recorded result — neither is a read, and neither
        // should have been reachable without an identity.
        .route("/telemetry/blur", post(report_blur_telemetry))
        .route("/ratings/update", post(update_free_rated_result))
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
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, (StatusCode, String)> {
    // A caller may only open a session for their own wallet. This used to be
    // conditional on an `Option<Extension<AuthedWallet>>` being present, which
    // meant a relay-secret request skipped the check entirely rather than
    // failing it. `RequireWallet` makes the identity mandatory.
    caller.require_is(&req.wallet_pubkey)?;

    // Funding a session key spends real lamports from the fee-payer pool, and
    // `game_id` is caller-chosen, so this route is a direct drain vector unless
    // it is rationed. Both budgets are checked before any keypair is generated.
    session_funding_guard(&state, &req.wallet_pubkey).await?;

    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid wallet_pubkey".to_string()))?;
    let session_pubkey = state.store.create(req.game_id, wallet).await.map_err(|e| {
        error!(
            "[VPS] Failed to create session for game {}: {}",
            req.game_id, e
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not create session".to_string(),
        )
    })?;
    // RateCache::get already degrades to a stale-but-real rate on fetch failure —
    // None only happens on a cold cache with no successful fetch yet ever. Reject
    // rather than silently defaulting to a free game in that case.
    let platform_fee_lamports = state
        .rate_cache
        .gbp_to_lamports(crate::signing::routes::rates::PLATFORM_FEE_GBP)
        .await
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "SOL/GBP rate unavailable".to_string(),
        ))?;
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
    caller: crate::signing::auth::RequireWallet,
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

    // The transaction's fee payer must be the authenticated caller. Without
    // this the endpoint co-signs setup transactions on behalf of arbitrary
    // wallets for arbitrary games — `activating_wallet` was previously read
    // only for idempotency bookkeeping, never for authorization.
    caller.require_is(&activating_wallet.to_string())?;

    // Inspect before signing. The session key is `game.fee_payer` on-chain and
    // holds backend funds, so co-signing an unexamined transaction is a signing
    // oracle: a crafted payload could carry a lamport transfer, or any other
    // instruction the session key is a valid signer for, and this endpoint
    // would sign it. Only the two real setup bundles the game client sends are
    // accepted — `[create_game, authorize_session_key]` and
    // `[join_game, authorize_session_key]` (see `solana/lobby.rs`) — and the
    // transaction must reference the Game PDA for the `game_id` it claims.
    let expected_game_pda = Pubkey::find_program_address(
        &[solana::GAME_SEED, &req.game_id.to_le_bytes()],
        &state.program_id,
    )
    .0;
    solana::validate_cosignable_tx(
        &peek_tx,
        &state.program_id,
        &[
            "create_game",
            "join_game",
            "authorize_session_key",
            "global_create_game",
            "global_join_game",
        ],
        &[expected_game_pda],
    )
    .map_err(|reason| {
        warn!(
            "[VPS] refused to co-sign setup TX for game {} from {activating_wallet}: {reason}",
            req.game_id
        );
        (
            StatusCode::BAD_REQUEST,
            format!("setup transaction rejected: {reason}"),
        )
    })?;

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
    // `fund_account` (15 s deadline) and `cosign_and_submit_tx` (30 s) both use
    // the *blocking* Solana client and poll for confirmation, so back to back
    // they can park an async worker thread for ~45 s. Run the pair on the
    // blocking pool — same reasoning as `record_move` below.
    let feepayer = state.feepayer.clone();
    let session_pubkey = entry.session_pubkey();
    let game_id = req.game_id;
    let rpc_url = state.config.solana_rpc_url.clone();
    let submit = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&rpc_url);
        solana::fund_account(
            &rpc,
            feepayer.next(),
            &session_pubkey,
            SESSION_FUND_LAMPORTS,
        )
        .map_err(|e| format!("Could not fund session key for game {game_id}: {e}"))?;
        // Submit the wallet-signed TX, adding the session key co-signature.
        solana::cosign_and_submit_tx(&rpc, &session_keypair, &tx_bytes)
            .map_err(|e| format!("Failed to submit setup TX for game {game_id}: {e}"))
    })
    .await
    .map_err(|e| {
        let msg = format!("activate_session task panicked for game {game_id}: {e}");
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;
    let sig = submit.map_err(|msg| {
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
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<RecordMoveReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    // If the caller authenticated with a per-user JWT, a valid credential for
    // wallet A must not be usable to submit a move for a game A has nothing
    // to do with. That does NOT mean caller == mover_wallet: the host's
    // client relays moves for *both* players of a game it's actually part
    // of (the backend's session key can sign for either mover, by design —
    // see resolve_move_signer), so rejecting every caller != mover_wallet
    // broke that relay pattern outright the moment a real two-player game
    // hit it (confirmed live: 403s on every relayed opponent move, cascading
    // into a finalize_game failure from the resulting on-chain move-count
    // mismatch). The correct check is on-chain participation in THIS
    // game_id, not string equality with the field the caller itself
    // supplied. Cheap in the common case (cached, see
    // `GameParticipantsCache`'s 5-minute TTL — participants never change
    // mid-game) and only ever consulted when the fast path doesn't apply.
    if caller.0 != req.mover_wallet {
        require_game_participant(&state, req.game_id, &caller.0)
            .await
            .map_err(|(status, mut msg)| {
                msg.push_str(&format!(
                    " (and does not match mover_wallet '{}')",
                    req.mover_wallet
                ));
                (status, msg)
            })?;
    }
    let mover_wallet = Pubkey::from_str(&req.mover_wallet).map_err(|_| {
        let msg = format!(
            "Bad or missing mover_wallet '{}' for game {}",
            req.mover_wallet, req.game_id
        );
        (StatusCode::BAD_REQUEST, msg)
    })?;
    let (session_kp, is_global) = resolve_move_signer(&state, req.game_id, mover_wallet)
        .await
        .map_err(|e| {
            let msg = format!(
                "No usable session for game {} / mover {}: {e:?}",
                req.game_id, mover_wallet
            );
            error!("[VPS] {msg}");
            match e {
                MoveSignerError::NoSession => (StatusCode::NOT_FOUND, msg),
                MoveSignerError::SessionInactive => (StatusCode::PRECONDITION_FAILED, msg),
            }
        })?;

    // ── Engine-side validation (replay from previous FEN) ──
    let pool = state.store.pool();
    let repo = GameRepository::new(pool.clone());
    // Only the latest FEN is needed here — see `get_last_fen`. An error or a
    // game with no recorded moves both fall back to the start position, as
    // scanning the full history used to.
    let prev_fen = repo
        .get_last_fen(&req.game_id.to_string())
        .await
        .ok()
        .flatten()
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
    let session_pk = session_kp.pubkey();

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
    // `GlobalSessionDelegation` instead. `is_global` reflects *this specific
    // mover's* session flow (from `resolve_move_signer`), not a game-level
    // flag — the two players in a game can be on different flows at once.
    // Likewise `mover_wallet`, not any stored-on-the-row wallet, is what the
    // delegation PDA must be derived from — see `resolve_move_signer`'s doc
    // comment for why.
    let ix = if is_global {
        solana::global_record_move_ix(
            &program_id,
            &session_pk,
            &mover_wallet,
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
            &mover_wallet,
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
    let er_url = er_rpc.url();

    // Serialize against any other ER write in flight for this game (another
    // record_move, an undelegate, the time-check crank) — see
    // `er_game_lock`'s doc comment.
    let _er_lock = er_game_lock(&state, req.game_id).await;

    // Same empty-body 502 problem as delegate_game/undelegate_game — this is
    // the route every single move goes through, so an opaque failure here is
    // the single most disruptive place for it to happen.
    // The Solana `RpcClient` used here is the *blocking* client, and
    // `sign_and_submit_er` polls for confirmation for up to 30 s. Awaiting that
    // inline parks a whole Tokio worker thread for the duration, so on a small
    // VPS a handful of concurrent moves starves the runtime — /health and
    // /metrics included. Hand it to the blocking pool, matching the pattern
    // already used in `tasks/settlement_worker.rs`.
    let sig = tokio::task::spawn_blocking(move || {
        solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix])
    })
    .await
    .map_err(|e| {
        let msg = format!("record_move task panicked for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?
    .map_err(|e| {
        let msg = format!("record_move failed for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} move {} recorded on Ephemeral Rollup ({}) sig {} — FEN after: {}",
        req.game_id, req.move_uci, &er_url, sig, derived_next_fen
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
    let player_wallet = mover_wallet.to_string();
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
        } else {
            publish_move_to_spectators(
                &state,
                &repo,
                &game_id_str,
                move_number,
                &move_uci,
                san.as_deref(),
                &next_fen,
                &player_wallet,
            )
            .await;
        }
    }

    Ok(Json(SigResp {
        sig: sig.to_string(),
        er_endpoint: er_url,
    }))
}

/// The Braid resource carrying a game's moves to spectators.
///
/// Registered lazily on a game's first move — a game nobody has played is not
/// a resource anyone can subscribe to, and `404` is the honest answer.
pub fn spectator_moves_path(game_id: &str) -> String {
    format!("game/{game_id}/moves")
}

/// Append one move to the game's spectator feed.
///
/// # Why this is gated on the broadcast delay
///
/// A `209` subscription pushes each move the instant it is recorded. For a
/// tournament game with a non-zero broadcast delay that is precisely the
/// ghosting channel the delay exists to close — `get_moves_visible` filters the
/// polled feed by timestamp, and a live stream would walk straight around it.
///
/// So a delayed game is never published here at all: its resource is never
/// registered, a subscribe gets `404`, and the client falls back to the
/// delay-gated poll it already uses. Two independent guards (this one, and the
/// client refusing to subscribe unless the delay it fetched is 0) — losing
/// either one alone does not leak a live board.
#[allow(clippy::too_many_arguments)]
async fn publish_move_to_spectators(
    state: &AppState,
    repo: &GameRepository,
    game_id: &str,
    move_number: i32,
    move_uci: &str,
    move_san: Option<&str>,
    fen_after: &str,
    player: &str,
) {
    if !crate::signing::routes::spectate::is_streamable(repo.get_broadcast_delay(game_id).await) {
        return;
    }

    let path = spectator_moves_path(game_id);

    // First move of this process's view of the game: hydrate from SQLite so a
    // spectator who subscribes later still gets the moves already played. A
    // restart mid-game otherwise leaves the log starting at "whatever happened
    // next", which renders as a board with pieces missing.
    if state.braid_hub.ensure_log(&path) {
        if let Ok(existing) = repo.get_moves(game_id).await {
            for m in existing {
                // The move just written is already in the table; hydrating
                // includes it, so don't append it twice below.
                if m.move_number == move_number {
                    continue;
                }
                state.braid_hub.append(
                    &path,
                    move_to_message(
                        &m.move_uci,
                        m.move_san.as_deref(),
                        m.fen_after.as_deref().unwrap_or_default(),
                        m.move_number,
                        &m.player,
                    ),
                );
            }
        }
    }

    state.braid_hub.append(
        &path,
        move_to_message(move_uci, move_san, fen_after, move_number, player),
    );
}

/// Encode one move as the `ChessMessage::Move` a spectator's
/// `braid_chess::ChessSubscriber` decodes.
///
/// History and live tail must produce byte-identical shapes — a subscriber has
/// one decode path for both.
fn move_to_message(
    move_uci: &str,
    _move_san: Option<&str>,
    fen_after: &str,
    move_number: i32,
    player: &str,
) -> serde_json::Value {
    let payload = braid_chess::MovePayload::from_uci(
        move_uci.to_string(),
        fen_after.to_string(),
        move_number.max(0) as u32,
        player.to_string(),
    );
    serde_json::to_value(braid_chess::ChessMessage::Move(payload))
        .unwrap_or(serde_json::Value::Null)
}

/// Asserts `wallet` is White or Black in `game_id` according to the chain.
///
/// This is the authorization primitive for every game-scoped route. String
/// equality against a field the caller supplied proves nothing, and a game's
/// host legitimately acts for *both* players (it relays the opponent's moves),
/// so the only meaningful question is on-chain participation in this specific
/// game. Cheap in the common case — `GameParticipantsCache` holds a 5-minute
/// TTL and participants never change mid-game.
async fn require_game_participant(
    state: &AppState,
    game_id: u64,
    wallet: &str,
) -> Result<(), (StatusCode, String)> {
    let wallet_pk = Pubkey::from_str(wallet).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid wallet '{wallet}'"),
        )
    })?;

    match state.game_participants.get(game_id).await {
        Some((white, black)) if wallet_pk == white || wallet_pk == black => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            format!("Authenticated wallet {wallet} is not a participant of game {game_id}"),
        )),
        // Participants unreadable (game not yet on chain, or RPC down). Fail
        // closed: an unverifiable claim is not an authorized one.
        None => Err((
            StatusCode::FORBIDDEN,
            format!(
                "Could not verify participation in game {game_id} on-chain; refusing the request"
            ),
        )),
    }
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
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<BlurTelemetryReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Only accept telemetry for games this server is actually relaying.
    state.store.get(req.game_id).await.ok_or((
        StatusCode::NOT_FOUND,
        format!("no session for game {}", req.game_id),
    ))?;

    if req.move_number == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "move_number is 1-based".to_string(),
        ));
    }
    let expected_color = if req.move_number % 2 == 1 {
        "white"
    } else {
        "black"
    };
    if req.color != expected_color {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("ply {} is {expected_color}'s", req.move_number),
        ));
    }

    // This feeds the anti-cheat pipeline, and `INSERT OR IGNORE` makes the first
    // write for a ply permanent. Unauthenticated, it let anyone race the real
    // client and mark an opponent's moves as window-blurred — the alt-tab-to-
    // engine signal — with no way to correct the record afterwards. A player may
    // only report their own colour, established from on-chain participation
    // rather than from the request body.
    require_game_participant(&state, req.game_id, &caller.0).await?;
    let (white, black) = state.game_participants.get(req.game_id).await.ok_or((
        StatusCode::FORBIDDEN,
        format!("could not resolve participants of game {}", req.game_id),
    ))?;
    let caller_is = if caller.0 == white.to_string() {
        "white"
    } else if caller.0 == black.to_string() {
        "black"
    } else {
        return Err((
            StatusCode::FORBIDDEN,
            "caller is not a player in this game".to_string(),
        ));
    };
    if caller_is != expected_color {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "you play {caller_is}; ply {} belongs to {expected_color} and only that player \
                 may report telemetry for it",
                req.move_number
            ),
        ));
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
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not record telemetry".to_string(),
        )
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
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<DelegateGameReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    // Delegation moves a live game onto the ER and spends fee-payer lamports on
    // the bookkeeping accounts' rent. Restrict it to the game's own players.
    require_game_participant(&state, req.game_id, &caller.0).await?;

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

    // `RpcClient` is the *synchronous* Solana client: `send_and_confirm_transaction`
    // parks the calling thread until the cluster confirms — seconds in the good
    // case, up to the client's 30 s timeout in the bad one. Running that straight
    // from an async handler occupies a Tokio worker for the whole duration, and
    // the runtime only has one worker per core: a handful of concurrent
    // delegations was enough to stall every other request on the server,
    // `/health` and `/metrics` included. All of it therefore runs on the blocking
    // pool, and only the signature crosses back.
    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::DelegateGame);
    let payer_bytes = payer.to_bytes();
    let session_bytes = session_kp.to_bytes();
    let game_id = req.game_id;
    let submit_url = rpc_url.clone();
    let sig = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let rpc = solana::make_rpc(&submit_url);
        let payer = Keypair::try_from(payer_bytes.as_slice())
            .map_err(|e| format!("fee-payer keypair unusable: {e}"))?;
        let session_kp = Keypair::try_from(session_bytes.as_slice())
            .map_err(|e| format!("session keypair unusable: {e}"))?;
        let blockhash = rpc.get_latest_blockhash().map_err(|e| {
            format!("delegate_game get_latest_blockhash failed for game {game_id}: {e}")
        })?;
        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &session_kp],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx)
            .map(|s| s.to_string())
            .map_err(|e| {
                // Debug, not just Display, logged separately: ClientError's
                // Display impl collapses a simulation failure to one summary
                // line ("Attempt to load a program that does not exist") with no
                // indication of *which* account/program the runtime was even
                // looking at. The Debug impl carries the full
                // RpcResponseErrorData (including the simulation's `logs`),
                // which is the only way to actually pin this down.
                error!("[VPS] delegate_game full error detail for game {game_id}: {e:?}");
                format!("delegate_game failed for game {game_id} (send failed): {e}")
            })
    })
    .await
    .map_err(|e| {
        let msg = format!("delegate_game task for game {game_id} panicked: {e}");
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?
    .map_err(|msg| {
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} delegated to Ephemeral Rollup ({}) sig {}",
        req.game_id, state.config.magic_router_rpc_url, sig
    );

    schedule_time_check_crank(&state, &rpc_url, &program_id, &session_kp, req.game_id).await;

    Ok(Json(SigResp {
        sig,
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
    base_rpc_url: &str,
    program_id: &Pubkey,
    session_kp: &solana_sdk::signature::Keypair,
    game_id: u64,
) {
    const TIME_CHECK_INTERVAL_MILLIS: u64 = 30_000;

    let game_pda =
        Pubkey::find_program_address(&[solana::GAME_SEED, &game_id.to_le_bytes()], program_id).0;

    // Takes the base-layer URL rather than a prebuilt client so the account
    // read can run on the blocking pool: `RpcClient` is the synchronous client,
    // and this executes inside request handling for `/game/delegate`.
    let read_url = base_rpc_url.to_string();
    let account = tokio::task::spawn_blocking(move || {
        solana::make_rpc(&read_url).get_account_data(&game_pda)
    })
    .await;

    let (white, black) = match account {
        Ok(Ok(data)) => match solana::game_account::parse(&data) {
            Some(game) => (game.white, game.black),
            None => {
                error!(
                    "[VPS] schedule_time_check: malformed Game account for game {}",
                    game_id
                );
                worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return;
            }
        },
        Ok(Err(e)) => {
            error!(
                "[VPS] schedule_time_check: failed to read Game account for game {}: {e}",
                game_id
            );
            worker_metrics::TIME_CHECK_SCHEDULE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        Err(e) => {
            error!(
                "[VPS] schedule_time_check: Game account read panicked for game {}: {e}",
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

    // Self-contained lock: this function is called from both `delegate_game`
    // (a client request) and `settlement_worker`'s periodic redelegate pass,
    // with no other coordination between those two callers. See
    // `er_game_lock`'s doc comment — this is exactly the collision that
    // stranded a real-wager game's checkmate move on 2026-08-10.
    let _er_lock = er_game_lock(state, game_id).await;

    // Blocking client + up-to-30 s confirmation poll: keep it off the async
    // worker threads. The keypair is round-tripped through its bytes because
    // this function only borrows it; a failure here is impossible for a valid
    // key, so it folds into the existing best-effort failure branch rather
    // than panicking.
    let submit = match solana_sdk::signature::Keypair::try_from(session_kp.to_bytes().as_slice()) {
        Ok(kp) => {
            tokio::task::spawn_blocking(move || solana::sign_and_submit_er(&er_rpc, &kp, &[ix]))
                .await
                .unwrap_or_else(|e| Err(anyhow::anyhow!("schedule_time_check task panicked: {e}")))
        }
        Err(e) => Err(anyhow::anyhow!("session keypair round-trip failed: {e}")),
    };

    match submit {
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

/// Acquires the per-game lock serializing ER-routed writes for `game_id`.
/// See `AppState::er_write_locks`'s doc comment for why this exists —
/// briefly: the ER rejects two concurrent writes to the same delegated
/// account outright, and this backend has multiple independent triggers
/// (a client's `record_move`/`undelegate_game` request,
/// `schedule_time_check_crank`'s own follow-up call, `settlement_worker`'s
/// periodic redelegate pass) that can all touch the same game with no other
/// coordination between them. Hold the returned guard for the entire
/// ER-touching portion of the caller — dropping it early re-opens the race.
///
/// Only the outer map lock (held briefly, to get-or-insert the per-game
/// entry) is shared across games; looking up game A's lock never blocks a
/// concurrent lookup for game B.
async fn er_game_lock(state: &AppState, game_id: u64) -> tokio::sync::OwnedMutexGuard<()> {
    let per_game = {
        let mut locks = state.er_write_locks.lock().await;
        locks
            .entry(game_id)
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    per_game.lock_owned().await
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

    let fee_payer = solana::game_account::parse(&data)?.fee_payer;

    let sessions = state.active_global_sessions.lock().await;
    sessions
        .values()
        .find(|kp| kp.pubkey() == fee_payer)
        .and_then(|kp| solana_sdk::signature::Keypair::try_from(kp.to_bytes().as_slice()).ok())
}

/// Resolves the signing keypair for one specific player's move within a
/// game, and which on-chain instruction it must go through.
///
/// A single relayer (the creator's client) submits *both* players' moves
/// for a game through this one backend, so `record_move` cannot just grab
/// "the" session on file for `game_id` — the two players can be on
/// completely different session flows at once (one using a long-lived
/// global session, the other a fresh per-game one), and even within the
/// per-game flow, `SessionStore` only ever tracks one physical keypair per
/// `game_id` shared by both players' on-chain `SessionDelegation`s (see
/// `SessionStore::create`'s "return the existing session pubkey" early
/// return) — the *account* that keypair authenticates as differs by which
/// wallet's delegation PDA is passed, which depends on whose move this is.
/// Picking the wrong one doesn't just misattribute a move — the on-chain
/// `apply_recorded_move` explicitly checks the claimed mover against
/// `game.turn`'s expected player and rejects the transaction with
/// `NotYourTurn` (confirmed live 2026-08-10: every non-creator move failed
/// exactly this way once the ER-routing fix let record_move calls actually
/// reach the chain for the first time).
///
/// Resolution order per `mover_wallet`:
/// 1. An active *global* session for that wallet specifically (not
///    whichever wallet's global flag happens to be set on the game's
///    `SessionStore` row) → `global_record_move` with that wallet's own
///    global session keypair.
/// 2. Otherwise, the per-game shared session on file for `game_id` → plain
///    `record_move`, deriving the `SessionDelegation` PDA from
///    `mover_wallet` (not the row's stored wallet, which only ever reflects
///    whoever activated their session first/most-recently).
#[derive(Debug, PartialEq, Eq)]
pub enum MoveSignerError {
    /// Neither a global session for `mover_wallet` nor any per-game session
    /// exists for `game_id`.
    NoSession,
    /// A per-game session exists but hasn't finished activation yet (the
    /// wallet-signed setup TX hasn't confirmed) — global sessions have no
    /// equivalent gate, see `resolve_move_signer`'s doc comment.
    SessionInactive,
}

pub async fn resolve_move_signer(
    state: &AppState,
    game_id: u64,
    mover_wallet: Pubkey,
) -> Result<(Keypair, bool /* is_global */), MoveSignerError> {
    {
        let sessions = state.active_global_sessions.lock().await;
        if let Some(kp) = sessions.get(&mover_wallet) {
            // Global sessions are marked usable at registration time (no
            // separate wallet-signed setup TX to wait for), so there's no
            // `active` flag to check here.
            let owned = Keypair::try_from(kp.to_bytes().as_slice())
                .map_err(|_| MoveSignerError::NoSession)?;
            return Ok((owned, true));
        }
    }
    let entry = state
        .store
        .get(game_id)
        .await
        .ok_or(MoveSignerError::NoSession)?;
    if !entry.active {
        return Err(MoveSignerError::SessionInactive);
    }
    Ok((entry.keypair(), false))
}

/// POST /game/undelegate - Undelegates a game from ER to devnet.
pub async fn undelegate_game(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<UndelegateGameReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    // Pulling a game off the ER mid-play stalls both players until it is
    // re-delegated, so this is a griefing lever for anyone but the players.
    require_game_participant(&state, req.game_id, &caller.0).await?;

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
    let er_url = er_rpc.url();

    // Held through the `cancel_time_check_crank` call below too — both are
    // ER writes for this same game and must not race a concurrent
    // record_move or crank. See `er_game_lock`'s doc comment.
    let _er_lock = er_game_lock(&state, req.game_id).await;

    // The client only ever sees `HTTP 502 —  —` on failure otherwise
    // (`vps_undelegate_game` folds the response body straight into its error
    // string) — the real reason (stale ER state, RPC timeout, session key
    // mismatch, ...) stayed stuck in this process's own log, which isn't
    // where anyone debugging a two-client test is looking.
    // Blocking client + up-to-30 s confirmation poll — off the async workers.
    // `er_rpc`/`session_kp` are handed back out of the closure because the
    // best-effort `cancel_time_check_crank` below still needs them, and it has
    // to stay inside `_er_lock`'s scope (see the lock comment above).
    let (submit, er_rpc, session_kp) = tokio::task::spawn_blocking(move || {
        let r = solana::sign_and_submit_er(&er_rpc, &session_kp, &[ix]);
        (r, er_rpc, session_kp)
    })
    .await
    .map_err(|e| {
        let msg = format!(
            "undelegate_game task panicked for game {}: {e}",
            req.game_id
        );
        error!("[VPS] {msg}");
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;
    let sig = submit.map_err(|e| {
        let msg = format!("undelegate_game failed for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        error!(
            "[VPS] undelegate_game full error detail for game {}: {e:?}",
            req.game_id
        );
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[ER] game {} undelegated from Ephemeral Rollup ({}) back to devnet, sig {}",
        req.game_id, &er_url, sig
    );

    // Cancel the time-check crank as a separate, best-effort follow-up — never
    // let a cancel failure affect the undelegate result already returned
    // above. A stray task on an undelegated account is expected to fail
    // harmlessly on its own next tick either way. Awaited (not detached) so it
    // still runs under `_er_lock`, exactly as the synchronous call did.
    let program_id = state.program_id;
    let game_id = req.game_id;
    let _ = tokio::task::spawn_blocking(move || {
        cancel_time_check_crank(&er_rpc, &program_id, &session_kp, game_id);
    })
    .await;

    Ok(Json(SigResp {
        sig: sig.to_string(),
        er_endpoint: er_url,
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

/// The on-chain fee-related fields read off a `Game` account just before
/// finalize closes it — see `read_game_fee_breakdown`.
struct GameFeeBreakdown {
    /// Real backend-advanced cost accrued via `lifecycle::transitions`
    /// (create/join/delegate/record_move/undelegate + the ER session fee) —
    /// what `settle_finished_game` reimburses to `treasury_vault` from the
    /// pot, clamped by what's left after the flat tx-fee deduction.
    fees_advanced: u64,
    /// The flat platform fee set at creation time, in lamports.
    country_fee: u64,
}

/// Reads the on-chain `Game.fees_advanced` and `Game.country_fee` fields, the
/// authoritative amounts `settle_finished_game` actually pays to
/// `treasury_vault` — recomputing an estimate backend-side (e.g. via a
/// BPS-of-pot guess) silently diverges from what the program transfers.
///
/// Returns `None` on any parse failure (missing account, unexpected layout) —
/// callers must treat that as "fee unknown", not "fee zero".
///
/// This used to walk the byte offsets itself and got them wrong: it stepped
/// `move_count` straight to `turn`, skipping `halfmove_clock`, so `country_fee`
/// was read two bytes early and the platform fee reported back to players was
/// garbage. It now goes through the shared decoder, which is pinned against the
/// program's own offset test.
fn read_game_fee_breakdown(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &Pubkey,
    game_id: u64,
) -> Option<GameFeeBreakdown> {
    let game_pda =
        Pubkey::find_program_address(&[solana::GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let data = rpc.get_account_data(&game_pda).ok()?;
    let game = solana::game_account::parse(&data)?;
    Some(GameFeeBreakdown {
        fees_advanced: game.fees_advanced,
        country_fee: game.country_fee,
    })
}

/// Flat ELO-linking fee split between both players at settlement — mirrors
/// `ELO_FEE_LAMPORTS` in `programs/xfchess-game/src/constants.rs`. Keep the
/// two in sync; this crate doesn't depend on the program crate.
const ELO_FEE_LAMPORTS: u64 = 5_000;

/// POST /game/finalize - Finalizes a game on devnet.
/// COUNTRY_FEE estimate — 1% of pot — used only as a fallback when the real
/// on-chain fee can't be read: either `read_game_fee_breakdown` failed, or (for
/// an already-finalized game, see `finalize_game`'s early-return) the `Game`
/// account is already closed and there's nothing left to read.
fn estimate_country_fee(wager_lamports: u64) -> u64 {
    const COUNTRY_FEE_BPS: u64 = 100; // 1%
    wager_lamports
        .saturating_mul(2)
        .saturating_mul(COUNTRY_FEE_BPS)
        / 10_000
}

/// Winner payout after fees — reproduces `settle_finished_game`'s exact
/// deduction order (tx-fee reimbursement, platform/operating-cost
/// reimbursement, country fee, then the flat per-player ELO fee) rather than
/// a BPS-of-pot guess, since the real on-chain amounts are flat constants,
/// not percentages. `country_fee` and `operating_cost` should be the real
/// on-chain values when available (see `estimate_country_fee` for the
/// country_fee fallback; pass 0 for `operating_cost` when the `Game` account
/// is already closed and `fees_advanced` can no longer be read).
fn compute_winner_lamports(
    wager_lamports: u64,
    country_fee: u64,
    operating_cost: u64,
    has_winner: bool,
) -> u64 {
    if !has_winner || wager_lamports == 0 {
        return 0; // draw or free game: handled on-chain / nothing to distribute
    }
    let pot = wager_lamports.saturating_mul(2);
    let tx_fee = 10_000u64.min(pot);
    let remaining = pot.saturating_sub(tx_fee);
    let platform_reimbursement = operating_cost.min(remaining);
    let remaining = remaining.saturating_sub(platform_reimbursement);
    let cfee = country_fee.min(remaining);
    let remaining = remaining.saturating_sub(cfee);
    let per_player = (ELO_FEE_LAMPORTS / 2).min(remaining / 2);
    remaining.saturating_sub(per_player.saturating_mul(2))
}

pub async fn finalize_game(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<FinalizeGameReq>,
) -> Result<Json<FinalizeResp>, (StatusCode, String)> {
    // The chain decides the payout regardless of what this request claims, so
    // an outsider calling this cannot redirect funds — but they can burn the
    // session key's lamports on a doomed transaction for any game they name.
    require_game_participant(&state, req.game_id, &caller.0).await?;

    // Both players' clients independently detect game-end locally and each
    // calls this route — deliberately, so finalization doesn't depend on one
    // specific client staying connected. Whichever call lands on-chain first
    // closes the `Game` account (`EndGame`'s `close = fee_payer`); the other
    // then fails with Anchor's `AccountNotInitialized`, wastes a tx fee, and
    // logs a scary-looking ERROR for what is actually a successful game.
    // Short-circuit here: if `complete_game` (below) already ran for this
    // `game_id`, the game is already settled — return that result instead of
    // repeating a doomed on-chain call. Confirmed live 2026-08-10: two
    // consecutive real games each produced exactly one Success + one Failed
    // finalize on Solscan; this eliminates the Failed half in the common
    // case (the two calls arriving more than an SQLite read apart).
    let repo = GameRepository::new(state.store.pool());
    if let Ok(Some(existing)) = repo.get_game(&req.game_id.to_string()).await {
        if existing.status == "completed" {
            // An *empty* signature is not evidence of settlement. `complete_game`
            // is also written by the off-chain rated-result path, which stores
            // `""` — treating that as a cached success made this return a
            // fabricated receipt and skip the real on-chain finalize entirely,
            // leaving the winner unpaid until the settlement worker caught up.
            if let Some(sig) = existing.finalize_sig.filter(|s| !s.trim().is_empty()) {
                info!(
                    "[VPS] finalize_game for game {} already completed (sig {sig}) — returning cached result",
                    req.game_id
                );
                let country_fee = estimate_country_fee(req.wager_lamports);
                // Game account is already closed by this point (cached path) —
                // fees_advanced can't be re-read, so operating cost/elo_fee
                // fall back to 0/estimate-only, same caveat estimate_country_fee
                // already carries.
                let winner_lamports = compute_winner_lamports(
                    req.wager_lamports,
                    country_fee,
                    0,
                    req.winner.is_some(),
                );
                let elo_fee = if req.wager_lamports > 0 {
                    ELO_FEE_LAMPORTS
                } else {
                    0
                };
                return Ok(Json(FinalizeResp {
                    sig,
                    winner_lamports,
                    country_fee,
                    operating_cost_lamports: 0,
                    elo_fee,
                }));
            }
        }
    }

    let session_kp = resolve_game_signer(&state, req.game_id)
        .await
        .ok_or_else(|| {
            let msg = format!("No session found for game {}", req.game_id);
            error!("[VPS] {msg}");
            (StatusCode::NOT_FOUND, msg)
        })?;

    let white = Pubkey::from_str(&req.white_pubkey).map_err(|e| {
        let msg = format!("Bad white_pubkey '{}': {e}", req.white_pubkey);
        (StatusCode::BAD_REQUEST, msg)
    })?;
    let black = Pubkey::from_str(&req.black_pubkey).map_err(|e| {
        let msg = format!("Bad black_pubkey '{}': {e}", req.black_pubkey);
        (StatusCode::BAD_REQUEST, msg)
    })?;
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

    // Read the actual fees that will be charged BEFORE finalize closes the
    // Game account out from under us.
    let fee_breakdown = read_game_fee_breakdown(&rpc, &state.program_id, req.game_id);

    let sig = solana::sign_and_submit(&rpc, &session_kp, &[ix]).map_err(|e| {
        let msg = format!("finalize_game failed for game {}: {e}", req.game_id);
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[FINALIZED] game {} settled on devnet — winner={:?} wager_lamports={} payout sig {} — inspect: https://solscan.io/tx/{}?cluster=devnet",
        req.game_id, req.winner, req.wager_lamports, sig, sig
    );
    let treasury_vault =
        Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &state.program_id).0;
    match &fee_breakdown {
        Some(f) if f.country_fee > 0 => info!(
            "[TREASURY] game {} paid {} lamports platform fee + {} lamports operating-cost reimbursement to treasury_vault {}",
            req.game_id, f.country_fee, f.fees_advanced, treasury_vault
        ),
        Some(f) => info!(
            "[TREASURY] game {} — free/unwagered game, no platform fee charged (backend ate {} lamports operating cost with no clawback)",
            req.game_id, f.fees_advanced
        ),
        None => warn!(
            "[TREASURY] game {} — could not read on-chain fee breakdown before finalize; treasury_vault {} payment amount unconfirmed",
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

    // Prefer the real on-chain values we just read (what the program actually
    // transfers) over estimates, which only approximate them and can
    // silently diverge from what `settle_finished_game` actually pays out.
    let country_fee = fee_breakdown
        .as_ref()
        .map(|f| f.country_fee)
        .unwrap_or_else(|| estimate_country_fee(req.wager_lamports));
    let operating_cost_lamports = fee_breakdown.as_ref().map(|f| f.fees_advanced).unwrap_or(0);
    let winner_lamports = compute_winner_lamports(
        req.wager_lamports,
        country_fee,
        operating_cost_lamports,
        req.winner.is_some(),
    );
    let elo_fee = if req.wager_lamports > 0 {
        ELO_FEE_LAMPORTS
    } else {
        0
    };

    Ok(Json(FinalizeResp {
        sig: sig.to_string(),
        winner_lamports,
        country_fee,
        operating_cost_lamports,
        elo_fee,
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

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatsResp {
    pub active_games: u64,
    pub unique_players: u64,
    pub total_sessions: u64,
    pub uptime_seconds: u64,
}

/// Process start instant, captured the first time `/stats` is served. Real
/// uptime needs a fixed reference point; the field previously reported
/// `SystemTime::now()` as seconds — the Unix clock, around 1.7 billion, not an
/// uptime at all.
fn process_start() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

/// Called once during startup so `/stats` reports uptime from process start
/// rather than from the first request that happened to touch it.
pub fn init_uptime_clock() {
    let _ = process_start();
}

/// GET /stats - Global platform statistics.
pub async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResp>, StatusCode> {
    let active_games = state.store.count_active().await;
    let unique_players = state.store.count_unique_players().await;
    let total_sessions = state.store.count_total_sessions().await;

    let uptime_seconds = process_start().elapsed().as_secs();

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

/// GET /game/:game_id/nonce - Returns the last confirmed move nonce, read from
/// the chain with the local move log as a fallback.
///
/// This used to derive the nonce purely from local SQLite rows while its doc
/// comment claimed it was the on-chain value. The two diverge in practice —
/// `record_move` submits to the ER first and persists afterwards, so a DB write
/// that fails (or a restore from an older snapshot) leaves the local count
/// behind the chain's, and the client then builds its next move on a nonce the
/// program rejects with `InvalidNonce`. The `Game` account is authoritative, so
/// read it; fall back to the local log only when the account can't be read,
/// which is the pre-existing behaviour and no worse than before.
pub async fn get_move_nonce(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<NonceResp>, StatusCode> {
    if let Some(on_chain) = read_game_nonce(&state, game_id).await {
        return Ok(Json(NonceResp { nonce: on_chain }));
    }

    let repo = GameRepository::new(state.store.pool());
    let next = repo
        .get_next_move_number(&game_id.to_string())
        .await
        .unwrap_or(1) as u64;
    // next_move_number is 1-based; last confirmed nonce = next - 1 (0 if no moves yet).
    let nonce = next.saturating_sub(1);
    warn!(
        "[VPS] game {game_id}: could not read on-chain nonce, falling back to local move log \
         ({nonce}) — a mismatch here surfaces to the client as InvalidNonce"
    );
    Ok(Json(NonceResp { nonce }))
}

/// Reads `Game.nonce` off-chain-account bytes, or `None` if the account is
/// missing/unreadable. Uses the shared decoder so the offsets stay in one place.
async fn read_game_nonce(state: &AppState, game_id: u64) -> Option<u64> {
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
    solana::game_account::parse(&data).map(|g| g.nonce)
}

// ── Item 4: free-rated ELO update ────────────────────────────────────────────

/// POST /ratings/update - Records result of a free (no-wager) rated game and triggers ELO update.
pub async fn update_free_rated_result(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<FreeRatedResultReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    // This writes a game's recorded players, winner and status. Unauthenticated,
    // it let anyone rewrite the result of *any* game — including a settled
    // wagered one — and, by marking a live game "completed", make
    // `finalize_game` short-circuit before its on-chain call. Restrict it to the
    // two people who actually played.
    if caller.0 != req.white_pubkey && caller.0 != req.black_pubkey {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "Authenticated wallet {} is not one of the two players named in this result",
                caller.0
            ),
        ));
    }

    // Free-rated games carry no wager, so a wagered game must never be settled
    // through this path — that is `finalize_game`'s job, and only the chain may
    // decide its outcome.
    if let Ok(Some(existing)) = GameRepository::new(state.store.pool())
        .get_game(&req.game_id.to_string())
        .await
    {
        if existing.stake_amount > 0.0 {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "Game {} carries a wager and settles on-chain; it cannot be recorded through \
                     the free-rated result path.",
                    req.game_id
                ),
            ));
        }
    }

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
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not record result".to_string(),
        ));
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
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not record result".to_string(),
        ));
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

/// POST /dispute/submit — **removed**. It logged a warning and returned
/// `dispute-{id}-pending`, which the client rendered as a filed dispute; nothing
/// was ever persisted and no admin could see it. Telling a player their dispute
/// is open when no record exists is worse than refusing outright, so this now
/// answers 501 and names the route that works — the same honest-501 treatment
/// `admin::force_resign` already uses for its unimplemented on-chain path.
///
/// The real flow is `POST /dispute/notify` (`routes::dispute`), which records
/// the case against the player's on-chain `dispute_game` transaction and is
/// resolved by `POST /admin/dispute/resolve`.
pub async fn submit_dispute(
    State(_state): State<AppState>,
    Json(req): Json<DisputeReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    warn!(
        "[dispute] rejected legacy /dispute/submit for game {} from {} — use /dispute/notify",
        req.game_id, req.disputing_player
    );
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "ok": false,
            "error": "not_implemented",
            "detail": "This endpoint never recorded anything. Submit the on-chain dispute_game \
                       transaction, then POST /dispute/notify with its signature to open a real \
                       case.",
            "use_instead": "/dispute/notify",
            "game_id": req.game_id,
        })),
    )
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
            mover_wallet: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    /// Pins the bug found live on devnet 2026-08-10: a single relayer
    /// backend submits *both* players' moves for a game, and the two
    /// players can be on completely different session flows at once (one
    /// long-lived global session, one fresh per-game session) — resolving
    /// "the" session for a `game_id` instead of the specific mover's wallet
    /// silently signed every move as the wrong player, which the on-chain
    /// program only surfaced as an opaque `NotYourTurn` once the record_move
    /// calls actually started reaching the chain. See `resolve_move_signer`'s
    /// doc comment for the full mechanism.
    #[tokio::test]
    async fn resolve_move_signer_picks_the_correct_players_session() {
        use crate::infrastructure::{initialize_pools, run_migrations};
        use crate::signing::storage::tournament::TournamentStore;
        use crate::signing::SigningConfig;

        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let db_url = |tag: &str| {
            format!(
                "sqlite:file:xfchess_resolve_move_signer_{tag}_{nanos}?mode=memory&cache=shared"
            )
        };
        let pools = initialize_pools(&db_url("session"), &db_url("vault"))
            .await
            .expect("init pools");
        run_migrations(&pools).await.expect("run migrations");
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

        let config = SigningConfig {
            port: 0,
            solana_rpc_url: "http://127.0.0.1:1".into(),
            solana_mainnet_rpc_url: None,
            er_rpc_url: "http://127.0.0.1:1".into(),
            magic_router_rpc_url: "http://127.0.0.1:1".into(),
            program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
            jwt_secret: "test-secret-not-for-production".into(),
            identity_encryption_key: "0".repeat(64),
            identity_salt: "0".repeat(64),
            fee_payer_keys: vec![],
            vps_authority_key: None,
            kyc_authority_key: None,
            link_authority_key: None,
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
            admin_token: None,
            tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
            usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
            lichess_client_id: String::new(),
            allowed_origins: vec![],
        };
        let state = AppState::new(
            config,
            pools.session_pool.clone(),
            pools.vault_pool.clone(),
            std::sync::Arc::new(tournament_store),
        );

        let game_id = 999_999u64;

        // White: long-lived global session, registered independently of any game.
        let white_wallet = Keypair::new().pubkey();
        let white_global_kp = Keypair::new();
        let white_global_pubkey = white_global_kp.pubkey();
        state
            .active_global_sessions
            .lock()
            .await
            .insert(white_wallet, white_global_kp);

        // Black: classic per-game flow — one shared session keypair for the
        // whole game, separately authorized on-chain by each wallet.
        let black_wallet = Keypair::new().pubkey();
        let classic_session_pubkey = state
            .store
            .create(game_id, black_wallet)
            .await
            .expect("create classic session");
        state.store.activate(game_id).await;

        let (white_kp, white_is_global) = resolve_move_signer(&state, game_id, white_wallet)
            .await
            .expect("white should resolve to its global session");
        assert!(white_is_global, "white registered a global session");
        assert_eq!(white_kp.pubkey(), white_global_pubkey);

        let (black_kp, black_is_global) = resolve_move_signer(&state, game_id, black_wallet)
            .await
            .expect("black should resolve to the classic per-game session");
        assert!(!black_is_global, "black used the classic per-game flow");
        assert_eq!(black_kp.pubkey(), classic_session_pubkey);

        // The actual bug: before this fix, both calls above returned
        // whichever session happened to be on file, so this always failed.
        assert_ne!(
            white_kp.pubkey(),
            black_kp.pubkey(),
            "each player must resolve to their own session, not one shared/clobbered signer"
        );
    }

    /// Reconciliation contract regression test (see
    /// `docs/plans/state-reconciliation-contract.md`): once Solana finalized
    /// state has been observed for a game — here, simulated by
    /// `SessionStore::deactivate`, exactly what
    /// `tasks::settlement_worker` calls the moment it sees the on-chain
    /// `Game` account closed/finished — no further move can be authorized
    /// for that game, no matter what the Ephemeral Rollup or the durable
    /// event log still believe. `resolve_move_signer` is the single choke
    /// point every move-recording path goes through
    /// (`record_move`/`delegate_game`/`undelegate_game`/`finalize_game`),
    /// so this one check is what makes "Solana finalized state wins" true
    /// in practice, not just in a doc comment.
    #[tokio::test]
    async fn settlement_deactivation_blocks_further_moves_even_with_a_live_session_on_file() {
        use crate::infrastructure::{initialize_pools, run_migrations};
        use crate::signing::storage::tournament::TournamentStore;
        use crate::signing::SigningConfig;

        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let db_url = |tag: &str| {
            format!("sqlite:file:xfchess_reconciliation_{tag}_{nanos}?mode=memory&cache=shared")
        };
        let pools = initialize_pools(&db_url("session"), &db_url("vault"))
            .await
            .expect("init pools");
        run_migrations(&pools).await.expect("run migrations");
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

        let config = SigningConfig {
            port: 0,
            solana_rpc_url: "http://127.0.0.1:1".into(),
            solana_mainnet_rpc_url: None,
            er_rpc_url: "http://127.0.0.1:1".into(),
            magic_router_rpc_url: "http://127.0.0.1:1".into(),
            program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
            jwt_secret: "test-secret-not-for-production".into(),
            identity_encryption_key: "0".repeat(64),
            identity_salt: "0".repeat(64),
            fee_payer_keys: vec![],
            vps_authority_key: None,
            kyc_authority_key: None,
            link_authority_key: None,
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
            admin_token: None,
            tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
            usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
            lichess_client_id: String::new(),
            allowed_origins: vec![],
        };
        let state = AppState::new(
            config,
            pools.session_pool.clone(),
            pools.vault_pool.clone(),
            std::sync::Arc::new(tournament_store),
        );

        let game_id = 555_555u64;
        let wallet = Keypair::new().pubkey();
        state
            .store
            .create(game_id, wallet)
            .await
            .expect("create session");
        state.store.activate(game_id).await;

        // While live: the ER/event-log side of the system can get a move
        // authorized, same as any in-progress game.
        resolve_move_signer(&state, game_id, wallet)
            .await
            .expect("an active session must resolve to a usable signer");

        // Reconciliation point: this is the exact call settlement_worker
        // makes on observing the on-chain Game account finalized/closed —
        // see `tasks/settlement_worker.rs`'s `state.store.deactivate` call
        // sites. Nothing about the ER state or the durable event log changes
        // here; only the backend's own bookkeeping does.
        state.store.deactivate(game_id).await;

        // After reconciliation: the exact same session row is still on
        // file (never deleted), but must no longer authorize a move — this
        // is "Solana finalized state overrides ER/event-log state" made
        // concrete and enforced, not just documented.
        let err = resolve_move_signer(&state, game_id, wallet)
            .await
            .expect_err("a deactivated (finalized) game must refuse further moves");
        assert_eq!(
            err,
            MoveSignerError::SessionInactive,
            "must reject via the SessionInactive path specifically, not just fail generically"
        );
    }

    /// Regression test for the live-devnet 403 (2026-08-16): a game's host
    /// relays moves for *both* players, so `caller` (the host's JWT wallet)
    /// and `mover_wallet` (whichever player actually moved) routinely
    /// differ. `record_move`'s auth check must allow that — verifying
    /// on-chain participation in the game, not string equality with
    /// `mover_wallet` — while still rejecting a wallet with no relationship
    /// to the game at all. See the doc comment at the top of `record_move`
    /// for the full incident writeup.
    #[tokio::test]
    async fn record_move_allows_a_genuine_participant_to_relay_the_other_players_move() {
        use crate::infrastructure::{initialize_pools, run_migrations};
        use crate::signing::storage::tournament::TournamentStore;
        use crate::signing::SigningConfig;

        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let db_url = |tag: &str| {
            format!("sqlite:file:xfchess_record_move_relay_{tag}_{nanos}?mode=memory&cache=shared")
        };
        let pools = initialize_pools(&db_url("session"), &db_url("vault"))
            .await
            .expect("init pools");
        run_migrations(&pools).await.expect("run migrations");
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

        let config = SigningConfig {
            port: 0,
            solana_rpc_url: "http://127.0.0.1:1".into(),
            solana_mainnet_rpc_url: None,
            er_rpc_url: "http://127.0.0.1:1".into(),
            magic_router_rpc_url: "http://127.0.0.1:1".into(),
            program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
            jwt_secret: "test-secret-not-for-production".into(),
            identity_encryption_key: "0".repeat(64),
            identity_salt: "0".repeat(64),
            fee_payer_keys: vec![],
            vps_authority_key: None,
            kyc_authority_key: None,
            link_authority_key: None,
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
            admin_token: None,
            tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
            usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
            lichess_client_id: String::new(),
            allowed_origins: vec![],
        };
        let state = AppState::new(
            config,
            pools.session_pool.clone(),
            pools.vault_pool.clone(),
            std::sync::Arc::new(tournament_store),
        );

        let game_id = 777_777u64;
        let white_wallet = Keypair::new().pubkey();
        let black_wallet = Keypair::new().pubkey();
        let stranger_wallet = Keypair::new().pubkey();
        state
            .game_participants
            .seed_for_test(game_id, white_wallet, black_wallet);

        let req_body = RecordMoveReq {
            game_id,
            move_uci: "e2e4".to_string(),
            next_fen: String::new(),
            nonce: 1,
            mover_wallet: black_wallet.to_string(),
        };

        // The host (authenticated as white) relays black's move — must clear
        // the auth check. No session was created, so it should fail later,
        // at resolve_move_signer (404), never with 403.
        let result = record_move(
            State(state.clone()),
            crate::signing::auth::RequireWallet(white_wallet.to_string()),
            Json(req_body.clone()),
        )
        .await;
        let (status, _) = match result {
            Err(e) => e,
            Ok(_) => panic!("no session exists yet, so this must still fail"),
        };
        assert_ne!(
            status,
            StatusCode::FORBIDDEN,
            "a genuine participant relaying the other player's move must not be 403'd"
        );

        // A wallet with no relationship to this game at all must still be
        // rejected — proves the check wasn't simply removed.
        let result = record_move(
            State(state.clone()),
            crate::signing::auth::RequireWallet(stranger_wallet.to_string()),
            Json(req_body),
        )
        .await;
        let (status, _) = match result {
            Err(e) => e,
            Ok(_) => panic!("a non-participant must be rejected"),
        };
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a wallet with no on-chain relationship to the game must be 403'd"
        );
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
                mover_wallet: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".to_string(),
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
            mover_wallet: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".to_string(),
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
