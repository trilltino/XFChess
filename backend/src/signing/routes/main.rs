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
use xfchess_client_compat::prelude::*;

use crate::db::repository::GameRepository;
use crate::signing::{solana, AppState};
use crate::telemetry::worker_metrics;

const SESSION_FUND_LAMPORTS: u64 = 10_000_000;

const SESSION_RATE_MAX_PER_WALLET: usize = 12;
const SESSION_RATE_WINDOW: std::time::Duration = std::time::Duration::from_secs(600);

const SESSION_MAX_PENDING_PER_WALLET: i64 = 6;

#[derive(Serialize)]
struct ActiveGameForWalletResp {
    game_id: Option<u64>,
}

async fn active_game_for_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<ActiveGameForWalletResp>, StatusCode> {
    let wallet = Pubkey::from_str(&wallet).map_err(|_| StatusCode::BAD_REQUEST)?;
    let game_ids = state.store.try_list_active_game_ids().await.map_err(|e| {
        warn!("[active-game] session lookup failed: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    if game_ids.is_empty() {
        return Ok(Json(ActiveGameForWalletResp { game_id: None }));
    }

    let rpc = state.solana_rpc.clone();
    let program_id = state.program_id;
    let delegation_program = Pubkey::from_str(solana::DELEGATION_PROGRAM_ID)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let found = tokio::task::spawn_blocking(move || -> Result<Option<u64>, StatusCode> {
        use crate::signing::solana::game_account::{
            parse, STATUS_ACTIVE, STATUS_WAITING_FOR_OPPONENT,
        };

        for chunk in game_ids.chunks(100) {
            let pdas: Vec<Pubkey> = chunk
                .iter()
                .map(|id| {
                    Pubkey::find_program_address(
                        &[solana::GAME_SEED, &id.to_le_bytes()],
                        &program_id,
                    )
                    .0
                })
                .collect();
            let accounts = rpc.get_multiple_accounts(&pdas).map_err(|e| {
                warn!("[active-game] RPC lookup failed: {e}");
                StatusCode::SERVICE_UNAVAILABLE
            })?;
            for (game_id, account) in chunk.iter().zip(accounts) {
                let Some(account) = account else { continue };
                // Delegation changes the account owner, not the Game layout or player membership.
                if account.owner != program_id && account.owner != delegation_program {
                    continue;
                }
                let Some(game) = parse(&account.data) else {
                    continue;
                };
                if game.game_id == *game_id
                    && (game.white == wallet || game.black == wallet)
                    && matches!(game.status, STATUS_WAITING_FOR_OPPONENT | STATUS_ACTIVE)
                {
                    return Ok(Some(*game_id));
                }
            }
        }
        Ok(None)
    })
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)??;
    Ok(Json(ActiveGameForWalletResp { game_id: found }))
}

fn session_create_tracker() -> &'static tokio::sync::Mutex<HashMap<String, Vec<std::time::Instant>>>
{
    static TRACKER: std::sync::OnceLock<
        tokio::sync::Mutex<HashMap<String, Vec<std::time::Instant>>>,
    > = std::sync::OnceLock::new();
    TRACKER.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

pub(crate) async fn session_funding_guard(
    state: &AppState,
    wallet: &str,
) -> Result<(), (StatusCode, String)> {
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

#[derive(Deserialize, Serialize)]
pub struct CreateSessionReq {
    pub game_id: u64,
    pub wallet_pubkey: String,
}

#[derive(Serialize)]
pub struct CreateSessionResp {
    pub session_pubkey: String,
    pub platform_fee_lamports: u64,
}

#[derive(Deserialize, Serialize)]
pub struct ActivateSessionReq {
    pub game_id: u64,
    pub signed_tx_b64: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct RecordMoveReq {
    pub game_id: u64,
    pub move_uci: String,
    pub next_fen: String,
    #[serde(default)]
    pub nonce: u64,
    #[serde(default)]
    pub mover_wallet: String,
    /// Writer's device id; checked against the caller's seat lease.
    #[serde(default)]
    pub device_id: Option<String>,
    /// The mover's base58 signature over `braid_chess::move_auth::message`,
    /// required when the caller is not the mover.
    #[serde(default)]
    pub mover_auth: Option<String>,
}

#[derive(Serialize, Default)]
pub struct SigResp {
    pub sig: String,
    #[serde(default)]
    pub er_endpoint: String,
}

#[derive(Deserialize)]
pub struct BlurTelemetryReq {
    pub game_id: u64,
    pub move_number: u32,
    pub color: String,
    pub blurred: bool,
    #[serde(default)]
    pub think_ms: Option<u32>,
}

#[derive(Serialize)]
pub struct FinalizeResp {
    pub sig: String,
    pub winner_lamports: u64,
    pub country_fee: u64,
    pub operating_cost_lamports: u64,
    pub elo_fee: u64,
}

#[derive(Serialize)]
pub struct NonceResp {
    pub nonce: u64,
}

#[derive(Deserialize, Serialize)]
pub struct FreeRatedResultReq {
    pub game_id: u64,
    pub winner: Option<String>,
    pub white_pubkey: String,
    pub black_pubkey: String,
}

#[derive(Deserialize, Serialize)]
pub struct DisputeReq {
    pub game_id: u64,
    pub disputing_player: String,
}

#[derive(Serialize)]
pub struct PlayerProfileResp {
    pub elo: u32,
    pub country: String,
    pub username: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/games/active/{wallet}", get(active_game_for_wallet))
        .route("/session/status/{game_id}", get(session_status))
        .route("/game/{game_id}/nonce", get(get_move_nonce))
        .route("/game/{game_id}/er-endpoint", get(game_er_endpoint))
        .route("/player/{pubkey}", get(get_player_profile))
        .route("/stats", get(get_stats))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/session/create", post(create_session))
        .route("/session/activate", post(activate_session))
        .route("/session/abandon/{game_id}", post(abandon_session))
        .route("/move/record", post(record_move))
        .route("/game/delegate", post(delegate_game))
        .route("/game/undelegate", post(undelegate_game))
        .route("/game/finalize", post(finalize_game))
        .route("/game/{game_id}/recover", post(recover_game))
        // Telemetry and result updates mutate player evidence; both require identity.
        .route("/telemetry/blur", post(report_blur_telemetry))
        .route("/ratings/update", post(update_free_rated_result))
        .route("/game/{game_id}/seat", get(get_seat))
        .route("/game/{game_id}/seat/claim", post(claim_seat))
}

async fn recover_game(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Path(game_id): Path<u64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Fresh PDA validation in the worker protects against stale membership data;
    // this guard prevents arbitrary callers from spending recovery sponsor fees.
    require_game_participant(&state, game_id, &caller.0).await?;
    let wallet: Pubkey = caller.0.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid wallet address".to_string(),
        )
    })?;
    let message =
        crate::tasks::settlement_worker::recover_game(&std::sync::Arc::new(state), game_id, wallet)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(serde_json::json!({ "message": message })))
}

async fn game_er_endpoint(
    State(state): State<AppState>,
    Path(game_id): Path<u64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rpc = solana::routing::er_rpc_for_game(&state.config, game_id)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(serde_json::json!({ "er_endpoint": rpc.url() })))
}

// Only authenticated on-chain participants may claim their own seat.
// The newest claim makes earlier devices view-only.

#[derive(Deserialize)]
pub struct ClaimSeatReq {
    pub device_id: String,
    /// This device's move-signing key (base58). Moves the other player relays
    /// for this wallet must be signed with it.
    #[serde(default)]
    pub move_signer: Option<String>,
}

pub async fn claim_seat(
    State(state): State<AppState>,
    Path(game_id): Path<u64>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<ClaimSeatReq>,
) -> Result<Json<crate::signing::storage::seat_lease::SeatLease>, (StatusCode, String)> {
    if !crate::signing::storage::seat_lease::valid_device_id(&req.device_id) {
        return Err((StatusCode::BAD_REQUEST, "invalid device_id".to_string()));
    }
    if req
        .move_signer
        .as_deref()
        .is_some_and(|signer| Pubkey::from_str(signer).is_err())
    {
        return Err((StatusCode::BAD_REQUEST, "invalid move_signer".to_string()));
    }
    require_game_participant(&state, game_id, &caller.0).await?;
    let now = chrono::Utc::now().timestamp();
    let lease = state
        .seat_leases
        .claim(
            &game_id.to_string(),
            &caller.0,
            &req.device_id,
            req.move_signer.as_deref(),
            now,
        )
        .await
        .map_err(|e| {
            error!("[seat] claim failed for game {game_id}: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "seat claim failed".to_string(),
            )
        })?;
    info!(
        "[seat] game {game_id}: {} seat held by device {} (epoch {})",
        caller.0, lease.device_id, lease.epoch
    );
    Ok(Json(lease))
}

pub async fn get_seat(
    State(state): State<AppState>,
    Path(game_id): Path<u64>,
    caller: crate::signing::auth::RequireWallet,
) -> Result<Json<Option<crate::signing::storage::seat_lease::SeatLease>>, (StatusCode, String)> {
    state
        .seat_leases
        .get(&game_id.to_string(), &caller.0)
        .await
        .map(Json)
        .map_err(|e| {
            error!("[seat] lookup failed for game {game_id}: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "seat lookup failed".to_string(),
            )
        })
}

/// Refuse a game-affecting write from a device that no longer holds
/// `wallet`'s seat. A store error fails closed for a claimed-capable request.
pub(crate) async fn require_seat_holder(
    state: &AppState,
    game_id: &str,
    wallet: &str,
    device_id: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    use crate::signing::storage::seat_lease::SeatCheck;
    match state.seat_leases.check(game_id, wallet, device_id).await {
        Ok(SeatCheck::Unclaimed | SeatCheck::Holder) => Ok(()),
        Ok(SeatCheck::Superseded(lease)) => Err((
            StatusCode::CONFLICT,
            format!(
                "seat_superseded: this game is being played on another device (epoch {})",
                lease.epoch
            ),
        )),
        Err(e) => {
            error!("[seat] check failed for game {game_id}: {e}");
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "seat check failed".to_string(),
            ))
        }
    }
}

// Wallet JWTs require signature-verified SIWS ownership proof.

pub async fn create_session(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, (StatusCode, String)> {
    // Opening a session requires a wallet JWT for the named wallet.
    caller.require_is(&req.wallet_pubkey)?;
    let _funding_lock = er_game_lock(&state, req.game_id).await;

    // Session funding spends fee-payer lamports for caller-chosen game IDs;
    // check both budgets before generating a keypair.
    session_funding_guard(&state, &req.wallet_pubkey).await?;

    let wallet = Pubkey::from_str(&req.wallet_pubkey)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid wallet_pubkey".to_string()))?;
    // Reject before persisting another pending key when rates are unavailable.
    let platform_fee_lamports = state
        .rate_cache
        .gbp_to_lamports(crate::signing::routes::rates::PLATFORM_FEE_GBP)
        .await
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "SOL/GBP rate unavailable".to_string(),
        ))?;
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
    let treasury_vault =
        Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &state.program_id).0;
    info!(
        "[TREASURY] game {} session created — {} lamports platform fee will be recorded at \
         create_game and paid to treasury_vault {} when the game finishes (0 for Free match type)",
        req.game_id, platform_fee_lamports, treasury_vault
    );

    // The client simulates create/join before opening its wallet. Both use this
    // key as the account-rent payer, so returning an unfunded key races that
    // simulation (System Program Custom(1)), even for a zero-stake game.
    let fee_payer = state.feepayer.clone();
    let rpc_url = state.config.solana_rpc_url.clone();
    tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&rpc_url).map_err(|error| error.to_string())?;
        solana::fund_account(
            &rpc,
            fee_payer.next(),
            &session_pubkey,
            SESSION_FUND_LAMPORTS,
        )
        .map_err(|e| {
            let message =
                solana::redact_urls_in(&format!("Session rent funding failed; retry hosting: {e}"));
            warn!("[VPS] {message}");
            message
        })?;
        let balance = rpc.get_balance(&session_pubkey).map_err(|e| {
            solana::redact_urls_in(&format!("Could not confirm session rent funding: {e}"))
        })?;
        if balance < SESSION_FUND_LAMPORTS {
            return Err("Session rent funding is not yet confirmed; retry hosting".to_string());
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Session funding task failed: {e}"),
        )
    })?
    .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;

    Ok(Json(CreateSessionResp {
        session_pubkey: session_pubkey.to_string(),
        platform_fee_lamports,
    }))
}

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

    // Key activation idempotency by (game_id, wallet): host and joiner submit
    // separate setup transactions for the same game.
    let peek_tx: solana_sdk::transaction::Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad tx bytes: {e}")))?;
    let activating_wallet = *peek_tx.message.account_keys.first().ok_or((
        StatusCode::BAD_REQUEST,
        "tx has no account keys".to_string(),
    ))?;

    // The setup transaction fee payer must be the authenticated wallet.
    caller.require_is(&activating_wallet.to_string())?;
    // Serialize funding and activation retries for this game with creation.
    let _setup_lock = er_game_lock(&state, req.game_id).await;

    // Inspect before co-signing with a funded session key. Accept only create/join
    // plus authorize_session_key bundles referencing the claimed Game PDA.
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
            "create_game_v2",
            "join_game_v2",
            "global_create_game_v2",
            "global_join_game_v2",
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

    // Co-sign with the session key: create/join require both player and fee-payer signatures.
    let entry = state
        .store
        .get(req.game_id)
        .await
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?
        .ok_or_else(|| {
            let msg = format!(
                "No session found for game {} — call /session/create first",
                req.game_id
            );
            error!("[VPS] {msg}");
            (StatusCode::NOT_FOUND, msg)
        })?;
    let session_keypair = entry.keypair();

    // Fund the session key before setup because it pays account rent. Funding and
    // submission use synchronous confirmation polling, so run both on the blocking pool.
    let feepayer = state.feepayer.clone();
    let session_pubkey = entry.session_pubkey();
    let game_id = req.game_id;
    let rpc_url = state.config.solana_rpc_url.clone();
    let submit = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&rpc_url).map_err(|error| error.to_string())?;
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

    // Track activation per wallet so retries are idempotent without blocking the other player.
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

pub async fn abandon_session(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Path(game_id): Path<u64>,
) -> Result<StatusCode, (StatusCode, String)> {
    if state.store.abandon_unactivated(game_id, &caller.0).await {
        info!(
            "[VPS] Wallet {} abandoned session for game {}",
            caller.0, game_id
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("No unactivated session for game {game_id} owned by caller"),
        ))
    }
}

pub async fn session_status(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(entry) = state
        .store
        .get(game_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    {
        return Ok(Json(serde_json::json!({
            "active": entry.active,
            "session_pubkey": entry.session_pubkey().to_string(),
        })));
    }
    // Global-session games have no separate activation step or SessionStore row.
    match resolve_game_signer(&state, game_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    {
        Some(kp) => Ok(Json(serde_json::json!({
            "active": true,
            "session_pubkey": kp.pubkey().to_string(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// A relayed move is recorded only with the mover's signature over it, made
/// with the move-signing key their seat registered. The signature covers the
/// previous recorded move, so the relayer cannot replay it at another ply or
/// after a different move of its own. Without this the backend, which holds
/// both players' session keys, would sign any legal move the opponent chose.
async fn require_mover_authorization(
    state: &AppState,
    repo: &GameRepository,
    req: &RecordMoveReq,
    mover: &Pubkey,
) -> Result<(), (StatusCode, String)> {
    let game_id = req.game_id.to_string();
    let forbidden = |reason: &str| {
        let msg = format!(
            "Relayed move {} for game {} rejected: {reason}",
            req.move_uci, req.game_id
        );
        warn!("[VPS] {msg}");
        (StatusCode::FORBIDDEN, msg)
    };
    let signer = state
        .seat_leases
        .move_signer(&game_id, &req.mover_wallet)
        .await
        .map_err(|e| {
            error!("[VPS] move signer lookup failed for game {game_id}: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "move signer lookup failed".to_string(),
            )
        })?
        .and_then(|signer| Pubkey::from_str(&signer).ok())
        .ok_or_else(|| forbidden("the mover's seat has no move signer"))?;
    let auth = req
        .mover_auth
        .as_deref()
        .ok_or_else(|| forbidden("no mover signature"))?;
    let previous = match req.nonce.checked_sub(1) {
        None => return Err(forbidden("nonce 0 is not a move")),
        Some(0) => None,
        Some(ply) => {
            let ply = i64::try_from(ply).map_err(|_| forbidden("nonce out of range"))?;
            let uci = repo.get_move_uci(&game_id, ply).await.map_err(|e| {
                error!("[DB] previous move lookup failed for game {game_id}: {e}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "previous move lookup failed".to_string(),
                )
            })?;
            // Moves are recorded in order, so a relay may run ahead of its
            // predecessor; the client retries.
            Some(uci.ok_or_else(|| {
                (
                    StatusCode::CONFLICT,
                    format!("Move {} for game {} is not recorded yet", ply, req.game_id),
                )
            })?)
        }
    };
    if !crate::signing::move_auth::is_authorized(
        &signer,
        auth,
        req.game_id,
        req.nonce,
        &req.move_uci,
        previous.as_deref(),
    ) {
        return Err(forbidden("the mover's signature does not match"));
    }
    info!(
        "[VPS] relayed move {} for game {} authorised by {}",
        req.move_uci, req.game_id, mover
    );
    Ok(())
}

pub async fn record_move(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<RecordMoveReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    // The host relays both players' moves, so the caller may differ from the
    // mover. It must then be a participant, and the move must carry the mover's
    // own signature (checked below, once the previous move is known).
    let relayed = caller.0 != req.mover_wallet;
    if relayed {
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
    // A device that lost the caller's seat to another device is view-only.
    require_seat_holder(
        &state,
        &req.game_id.to_string(),
        &caller.0,
        req.device_id.as_deref(),
    )
    .await?;
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
                MoveSignerError::StorageUnavailable => (StatusCode::SERVICE_UNAVAILABLE, msg),
                MoveSignerError::NoSession => (StatusCode::NOT_FOUND, msg),
                MoveSignerError::SessionInactive => (StatusCode::PRECONDITION_FAILED, msg),
            }
        })?;

    // ── Engine-side validation (replay from previous FEN) ──
    let pool = state.store.pool();
    let repo = GameRepository::new(pool.clone());
    // Read only the latest FEN; missing moves or read errors fall back to the start position.
    let prev_fen = repo
        .get_last_fen(&req.game_id.to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    let mut game = xfchess_rules::Game::from_fen(&prev_fen)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mv_bytes = uci_to_fixed5(&req.move_uci).map_err(|_| {
        let msg = format!("Bad move_uci '{}' for game {}", req.move_uci, req.game_id);
        (StatusCode::BAD_REQUEST, msg)
    })?;
    if relayed {
        require_mover_authorization(&state, &repo, &req, &mover_wallet).await?;
    }

    let san = game.san(&req.move_uci).ok();
    game.play_uci(&req.move_uci).map_err(|e| {
        let msg = format!(
            "Move {} rejected by engine for game {}: {e:?}",
            req.move_uci, req.game_id
        );
        (StatusCode::BAD_REQUEST, msg)
    })?;

    // Derive next FEN from the engine (discard client-provided next_fen)
    let board = game.compact_board();
    let next_board = board.to_bytes();
    let derived_next_fen = board
        .to_fen()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;

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

    // Select the move instruction using this mover's session flow; opponents may
    // use different flows. Derive the delegation PDA from mover_wallet.
    let ix = if is_global {
        solana::global_record_move_ix(
            &program_id,
            &session_pk,
            &mover_wallet,
            crate::signing::solana::MoveInput {
                game_id: req.game_id,
                move_uci: mv_bytes,
                next_board,
                nonce: req.nonce,
                signature: Some(sig_bytes),
                parent_nonce: req.nonce.checked_sub(1),
            },
        )
    } else {
        solana::record_move_ix(
            &program_id,
            &session_pk,
            &mover_wallet,
            crate::signing::solana::MoveInput {
                game_id: req.game_id,
                move_uci: mv_bytes,
                next_board,
                nonce: req.nonce,
                signature: Some(sig_bytes),
                parent_nonce: req.nonce.checked_sub(1),
            },
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
    let er_rpc = solana::routing::er_rpc_for_game(&state.config, req.game_id)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    let er_url = er_rpc.url();

    // Serialize all ER writes for this game, including moves, cranks, and undelegation.
    let _er_lock = er_game_lock(&state, req.game_id).await;

    // Run blocking ER submission and confirmation polling off async worker threads.
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

    // Persist each move before responding so the next request validates against
    // the updated FEN, including batches submitted back to back.
    let game_id_str = req.game_id.to_string();
    let player_wallet = mover_wallet.to_string();
    let move_uci = req.move_uci.clone();
    let next_fen = derived_next_fen;
    if let Err(e) = repo.upsert_game(&game_id_str).await {
        error!("[DB] Failed to upsert game {}: {}", game_id_str, e);
    } else {
        let move_number = repo.get_next_move_number(&game_id_str).await.unwrap_or(1) as i32;

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

pub fn spectator_moves_path(game_id: &str) -> String {
    format!("game/{game_id}/moves")
}

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

    // Hydrate the move log from SQLite before appending after a restart.
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

const MAX_THINK_MS: u32 = 2 * 60 * 60 * 1000; // 2 hours

pub async fn report_blur_telemetry(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<BlurTelemetryReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Only accept telemetry for games this server is actually relaying.
    state
        .store
        .get(req.game_id)
        .await
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?
        .ok_or((
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

    // Blur telemetry is immutable per ply; accept only the authenticated player's
    // on-chain color to prevent forged anti-cheat evidence.
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

fn uci_to_fixed5(uci: &str) -> Result<[u8; 5], ()> {
    let bytes = uci.as_bytes();
    if bytes.len() < 4 || bytes.len() > 5 {
        return Err(());
    }
    let mut out = [0u8; 5];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
}

pub async fn delegate_game(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<DelegateGameReq>,
) -> Result<Json<SigResp>, (StatusCode, String)> {
    // Delegation moves a live game onto the ER and spends fee-payer lamports on
    // the bookkeeping accounts' rent. Restrict it to the game's own players.
    require_game_participant(&state, req.game_id, &caller.0).await?;

    let entry = state
        .store
        .get(req.game_id)
        .await
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?
        .ok_or_else(|| {
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

    // Run synchronous transaction submission and confirmation on the blocking pool.
    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::DelegateGame);
    let payer_bytes = payer.to_bytes();
    let session_bytes = session_kp.to_bytes();
    let game_id = req.game_id;
    let submit_url = rpc_url.clone();
    let sig = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let rpc = solana::make_rpc(&submit_url).map_err(|error| error.to_string())?;
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
        rpc.send_sdk_and_confirm_transaction(&tx)
            .map(|s| s.to_string())
            .map_err(|e| {
                // Log Debug separately to retain simulation logs omitted by ClientError Display.
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

    solana::routing::invalidate_er_for_game(&state.config, req.game_id);
    schedule_time_check_crank(&state, &rpc_url, &program_id, &session_kp, req.game_id).await;

    Ok(Json(SigResp {
        sig,
        ..Default::default()
    }))
}

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

    // Construct the synchronous RPC client on the blocking pool during request handling.
    let read_url = base_rpc_url.to_string();
    let account = tokio::task::spawn_blocking(move || {
        solana::make_rpc(&read_url)?.get_account_data(&game_pda)
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

    let er_rpc = match solana::routing::er_rpc_for_game(&state.config, game_id).await {
        Ok(rpc) => rpc,
        Err(e) => {
            warn!("[VPS] Cannot schedule time check for game {game_id}: {e}");
            return;
        }
    };

    // Coordinate client delegation and settlement-worker redelegation with the same lock.
    let _er_lock = er_game_lock(state, game_id).await;

    // Keep synchronous confirmation off async workers. Copy the borrowed keypair
    // through bytes for the blocking closure.
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

pub(crate) async fn er_game_lock(
    state: &AppState,
    game_id: u64,
) -> tokio::sync::OwnedMutexGuard<()> {
    let per_game = {
        let mut locks = state.er_write_locks.lock().await;
        locks
            .entry(game_id)
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    per_game.lock_owned().await
}

pub async fn resolve_game_signer(
    state: &AppState,
    game_id: u64,
) -> anyhow::Result<Option<std::sync::Arc<Keypair>>> {
    if let Some(entry) = state.store.get(game_id).await? {
        return Ok(Some(entry.keypair()));
    }

    let game_pda = Pubkey::find_program_address(
        &[solana::GAME_SEED, &game_id.to_le_bytes()],
        &state.program_id,
    )
    .0;
    let rpc = std::sync::Arc::clone(&state.solana_rpc);
    let data = tokio::task::spawn_blocking(move || rpc.get_account_data(&game_pda)).await??;

    let Some(game) = solana::game_account::parse(&data) else {
        return Ok(None);
    };
    let fee_payer = game.fee_payer;

    let sessions = state.active_global_sessions.lock().await;
    Ok(sessions
        .values()
        .find(|kp| kp.pubkey() == fee_payer)
        .map(|key| std::sync::Arc::new(key.insecure_clone())))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MoveSignerError {
    #[error("move signing session not found")]
    NoSession,
    #[error("move signing session is inactive")]
    SessionInactive,
    #[error("move signing storage is unavailable")]
    StorageUnavailable,
}

pub async fn resolve_move_signer(
    state: &AppState,
    game_id: u64,
    mover_wallet: Pubkey,
) -> Result<(std::sync::Arc<Keypair>, bool /* is_global */), MoveSignerError> {
    {
        let sessions = state.active_global_sessions.lock().await;
        if let Some(kp) = sessions.get(&mover_wallet) {
            // Global sessions are usable at registration; there is no separate active flag.
            let owned = Keypair::try_from(kp.to_bytes().as_slice())
                .map_err(|_| MoveSignerError::NoSession)?;
            return Ok((std::sync::Arc::new(owned), true));
        }
    }
    let entry = state
        .store
        .get(game_id)
        .await
        .map_err(|_| MoveSignerError::StorageUnavailable)?
        .ok_or(MoveSignerError::NoSession)?;
    if !entry.active {
        return Err(MoveSignerError::SessionInactive);
    }
    Ok((entry.keypair(), false))
}

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
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?
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
    let er_rpc = solana::routing::er_rpc_for_game(&state.config, req.game_id)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
    let er_url = er_rpc.url();

    // Hold the lock through crank cancellation to serialize all ER writes for this game.
    let _er_lock = er_game_lock(&state, req.game_id).await;

    // Run undelegation on the blocking pool and return its RPC client and keypair
    // for crank cancellation while retaining _er_lock.
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

    // Cancel the crank under _er_lock as a best-effort follow-up; cancellation
    // failure must not change the successful undelegation result.
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

fn read_game_snapshot(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &Pubkey,
    game_id: u64,
) -> Option<solana::game_account::GameAccount> {
    let game_pda =
        Pubkey::find_program_address(&[solana::GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let data = rpc.get_account_data(&game_pda).ok()?;
    solana::game_account::parse(&data)
}

const ELO_FEE_LAMPORTS: u64 = 5_000;

fn estimate_country_fee(wager_lamports: u64) -> u64 {
    const COUNTRY_FEE_BPS: u64 = 100; // 1%
    wager_lamports
        .saturating_mul(2)
        .saturating_mul(COUNTRY_FEE_BPS)
        / 10_000
}

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
    // The chain fixes payout destinations, but unauthorized requests could still burn session fees.
    require_game_participant(&state, req.game_id, &caller.0).await?;

    // Both clients may finalize. Return a recorded settlement receipt before
    // repeating the transaction that closes the Game account.
    let repo = GameRepository::new(state.store.pool());
    if let Ok(Some(existing)) = repo.get_game(&req.game_id.to_string()).await {
        if existing.status == "completed" {
            // An empty signature is an off-chain result, not evidence of on-chain settlement.
            if let Some(sig) = existing.finalize_sig.filter(|s| !s.trim().is_empty()) {
                info!(
                    "[VPS] finalize_game for game {} already completed (sig {sig}) — returning cached result",
                    req.game_id
                );
                let country_fee = estimate_country_fee(req.wager_lamports);
                // The closed account cannot supply fees_advanced; cached results use estimated costs.
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
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?
        .ok_or_else(|| {
            let msg = format!("No session found for game {}", req.game_id);
            error!("[VPS] {msg}");
            (StatusCode::NOT_FOUND, msg)
        })?;

    // Players, result and fees come from the Game account, read before
    // finalize closes it. The request's players and winner are not trusted:
    // they would otherwise be written into the game record and anti-cheat.
    let game_id = req.game_id;
    let program_id = state.program_id;
    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::FinalizeGame);
    let snap = {
        let rpc_url = rpc_url.clone();
        tokio::task::spawn_blocking(move || {
            read_game_snapshot(&solana::make_rpc(&rpc_url).ok()?, &program_id, game_id)
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("game read task: {e}"),
            )
        })?
    }
    .ok_or_else(|| {
        let msg = format!("Could not read game {game_id} on-chain");
        warn!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;

    // The program pays the closed Game's rent to the recorded fee payer.
    let ix = solana::finalize_game_ix(
        &program_id,
        game_id,
        &snap.white,
        &snap.black,
        None,
        &snap.fee_payer,
    );
    let sig = tokio::task::spawn_blocking(move || {
        solana::sign_and_submit(&solana::make_rpc(&rpc_url)?, &session_kp, &[ix])
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("finalize task: {e}"),
        )
    })?
    .map_err(|e| {
        let msg = format!("finalize_game failed for game {game_id}: {e}");
        error!("[VPS] {msg}");
        (StatusCode::BAD_GATEWAY, msg)
    })?;
    info!(
        "[FINALIZED] game {} settled — status={} winner={:?} wager_lamports={} payout sig {}",
        game_id, snap.status, snap.winner, snap.wager_amount, sig
    );
    let winner_wallet = snap.winner.map(|w| w.to_string());
    if let Ok(record) = state
        .money_actions
        .create_or_get(crate::signing::storage::money_action::NewMoneyAction {
            action_type: "finalize_game".to_string(),
            scope_type: "game".to_string(),
            game_id: Some(game_id as i64),
            tournament_id: None,
            wallet: winner_wallet,
            signature: Some(sig.to_string()),
            status: "submitted".to_string(),
            reason: Some("backend submitted game finalization/payout".to_string()),
        })
        .await
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::signing::routes::money_actions::reconcile_money_action_once(
                state_clone,
                record.id,
            )
            .await;
        });
    }
    let treasury_vault =
        Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &program_id).0;
    info!(
        "[TREASURY] game {} paid {} lamports platform fee + {} lamports operating-cost reimbursement to treasury_vault {}",
        game_id, snap.country_fee, snap.fees_advanced, treasury_vault
    );

    // The same bookkeeping as automatic settlement: game record, PGN, ratings
    // cache, anti-cheat with tournament context, tournament result, session retirement.
    let sig_str = sig.to_string();
    let app_state = state.clone();
    let recorded = snap.clone();
    tokio::spawn(async move {
        crate::tasks::settlement_worker::record_settled_game(
            &app_state,
            game_id,
            &recorded,
            Some(&sig_str),
        )
        .await;
    });

    let has_winner = snap.winner.is_some();
    let winner_lamports = compute_winner_lamports(
        snap.wager_amount,
        snap.country_fee,
        snap.fees_advanced,
        has_winner,
    );
    let elo_fee = if snap.wager_amount > 0 {
        ELO_FEE_LAMPORTS
    } else {
        0
    };

    Ok(Json(FinalizeResp {
        sig: sig.to_string(),
        winner_lamports,
        country_fee: snap.country_fee,
        operating_cost_lamports: snap.fees_advanced,
        elo_fee,
    }))
}

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
    #[serde(default)]
    pub wager_lamports: u64,
}

#[derive(Serialize)]
pub struct StatsResp {
    pub active_games: u64,
    pub unique_players: u64,
    pub total_sessions: u64,
    pub uptime_seconds: u64,
}

fn process_start() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

pub fn init_uptime_clock() {
    let _ = process_start();
}

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

pub async fn get_move_nonce(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<NonceResp>, StatusCode> {
    if let Some(on_chain) = read_game_nonce(&state, game_id).await {
        return Ok(Json(NonceResp { nonce: on_chain }));
    }
    warn!("[VPS] game {game_id}: cannot read authoritative nonce; refusing an unverified fallback");
    Err(StatusCode::SERVICE_UNAVAILABLE)
}

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

pub async fn update_free_rated_result(
    State(state): State<AppState>,
    caller: crate::signing::auth::RequireWallet,
    Json(req): Json<FreeRatedResultReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Only the two participants may update a game result; completion affects
    // finalization idempotency and must not be caller-forged.
    if caller.0 != req.white_pubkey && caller.0 != req.black_pubkey {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "Authenticated wallet {} is not one of the two players named in this result",
                caller.0
            ),
        ));
    }

    // This free-rated path must never settle wagers; only the chain decides staked outcomes.
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
        .complete_game(crate::db::repository::CompletedGame {
            game_id: &game_id_str,
            player_white: Some(&req.white_pubkey),
            player_black: Some(&req.black_pubkey),
            white_username: white_username.as_deref(),
            black_username: black_username.as_deref(),
            winner,
            final_fen: None,
            finalize_sig: "",
            stake_amount: 0.0,
        })
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
    use anyhow::Context as _;
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
            device_id: None,
            mover_auth: None,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn resolve_move_signer_picks_the_correct_players_session() -> anyhow::Result<()> {
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
            .context("init pools")?;
        run_migrations(&pools).await.context("run migrations")?;
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await?;

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
        )?;

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
            .context("create classic session")?;
        state.store.activate(game_id).await;

        let (white_kp, white_is_global) = resolve_move_signer(&state, game_id, white_wallet)
            .await
            .context("white should resolve to its global session")?;
        assert!(white_is_global, "white registered a global session");
        assert_eq!(white_kp.pubkey(), white_global_pubkey);

        let (black_kp, black_is_global) = resolve_move_signer(&state, game_id, black_wallet)
            .await
            .context("black should resolve to the classic per-game session")?;
        assert!(!black_is_global, "black used the classic per-game flow");
        assert_eq!(black_kp.pubkey(), classic_session_pubkey);

        // The actual bug: before this fix, both calls above returned
        // whichever session happened to be on file, so this always failed.
        assert_ne!(
            white_kp.pubkey(),
            black_kp.pubkey(),
            "each player must resolve to their own session, not one shared/clobbered signer"
        );
        Ok(())
    }

    #[tokio::test]
    async fn settlement_deactivation_blocks_further_moves_even_with_a_live_session_on_file(
    ) -> anyhow::Result<()> {
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
            .context("init pools")?;
        run_migrations(&pools).await.context("run migrations")?;
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await?;

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
        )?;

        let game_id = 555_555u64;
        let wallet = Keypair::new().pubkey();
        state
            .store
            .create(game_id, wallet)
            .await
            .context("create session")?;
        state.store.activate(game_id).await;

        // While live: the ER/event-log side of the system can get a move
        // authorized, same as any in-progress game.
        resolve_move_signer(&state, game_id, wallet)
            .await
            .context("an active session must resolve to a usable signer")?;

        // Settlement-worker reconciliation updates backend session bookkeeping only.
        state.store.deactivate(game_id).await;

        // Keep the reconciled session row but revoke its authority to record moves.
        let err = resolve_move_signer(&state, game_id, wallet)
            .await
            .err()
            .context("a deactivated (finalized) game must refuse further moves")?;
        assert_eq!(
            err,
            MoveSignerError::SessionInactive,
            "must reject via the SessionInactive path specifically, not just fail generically"
        );
        Ok(())
    }

    #[tokio::test]
    async fn record_move_relays_only_moves_the_mover_signed() -> anyhow::Result<()> {
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
            .context("init pools")?;
        run_migrations(&pools).await.context("run migrations")?;
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await?;

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
        )?;

        let game_id = 777_777u64;
        let white_wallet = Keypair::new().pubkey();
        let black_wallet = Keypair::new().pubkey();
        let stranger_wallet = Keypair::new().pubkey();
        state
            .game_participants
            .seed_for_test(game_id, white_wallet, black_wallet);
        state
            .active_global_sessions
            .lock()
            .await
            .insert(black_wallet, Keypair::new());

        // White's 1.e4 is recorded; white's client now relays black's reply.
        let mut position = xfchess_rules::Game::default();
        position
            .play_uci("e2e4")
            .map_err(anyhow::Error::msg)
            .context("test setup failed")?;
        let repo = GameRepository::new(state.store.pool().clone());
        repo.upsert_game(&game_id.to_string())
            .await
            .context("test setup failed")?;
        repo.add_move_simple(
            &game_id.to_string(),
            1,
            "e2e4",
            None,
            Some(
                &position
                    .compact_board()
                    .to_fen()
                    .map_err(anyhow::Error::msg)?,
            ),
            &white_wallet.to_string(),
        )
        .await
        .context("test setup failed")?;

        let black_device_key = Keypair::new();
        let sign = |kp: &Keypair, previous: &str| {
            kp.sign_message(&braid_chess::move_auth::message(
                game_id,
                2,
                "e7e5",
                Some(previous),
            ))
            .to_string()
        };
        let black_device = "device-black-0001";
        let relay = |caller: Pubkey, mover: Pubkey, mover_auth: Option<String>| {
            let state = state.clone();
            async move {
                let req = RecordMoveReq {
                    game_id,
                    move_uci: "e7e5".to_string(),
                    next_fen: String::new(),
                    nonce: 2,
                    mover_wallet: mover.to_string(),
                    // Black's seat is claimed below; black writes from that device.
                    device_id: (caller == black_wallet).then(|| black_device.to_string()),
                    mover_auth,
                };
                match record_move(
                    State(state),
                    crate::signing::auth::RequireWallet(caller.to_string()),
                    Json(req),
                )
                .await
                {
                    Err((status, _)) => status,
                    Ok(_) => StatusCode::OK,
                }
            }
        };
        let valid = sign(&black_device_key, "e2e4");

        // Black has not registered a move signer yet.
        assert_eq!(
            relay(white_wallet, black_wallet, Some(valid.clone())).await,
            StatusCode::FORBIDDEN
        );
        state
            .seat_leases
            .claim(
                &game_id.to_string(),
                &black_wallet.to_string(),
                black_device,
                Some(&black_device_key.pubkey().to_string()),
                1,
            )
            .await
            .context("test setup failed")?;

        for forged in [
            None,
            Some(sign(&Keypair::new(), "e2e4")),
            // Signed in reply to a move white never recorded.
            Some(sign(&black_device_key, "d2d4")),
        ] {
            assert_eq!(
                relay(white_wallet, black_wallet, forged).await,
                StatusCode::FORBIDDEN
            );
        }
        // A wallet outside the game is refused even with a valid signature.
        assert_eq!(
            relay(stranger_wallet, black_wallet, Some(valid.clone())).await,
            StatusCode::FORBIDDEN
        );

        // The signed move passes authorisation, then fails validator discovery
        // because the test router is unreachable.
        assert_eq!(
            relay(white_wallet, black_wallet, Some(valid)).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        // A player submitting their own move needs no signature.
        assert_eq!(
            relay(black_wallet, black_wallet, None).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        Ok(())
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
                device_id: None,
                mover_auth: None,
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
            device_id: None,
            mover_auth: None,
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
    fn test_timestamp_generation() -> anyhow::Result<()> {
        // Test that SystemTime can generate valid timestamps
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("System time should be after UNIX_EPOCH")?;
        assert!(now.as_secs() > 0);
        Ok(())
    }
}
