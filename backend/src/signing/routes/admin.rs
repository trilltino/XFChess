use crate::db::repository::GameRepository;
use crate::signing::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

// ── In-memory state for features that don't yet have DB backing ──────────────

#[derive(Clone, Serialize)]
struct BanEntry {
    wallet: String,
    reason: String,
    duration_days: Option<u32>,
    banned_at: u64,
}

#[derive(Clone, Serialize)]
struct IpBanEntry {
    ip: String,
    reason: String,
    banned_at: u64,
}

#[derive(Clone, Serialize)]
struct AuditEntry {
    timestamp: u64,
    actor: String,
    action: String,
    target: String,
    result: String,
}

static PLAYER_BANS: Lazy<Mutex<HashMap<String, BanEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static ELO_OVERRIDES: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static IP_BANS: Lazy<Mutex<Vec<IpBanEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));
static WHITELIST: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));
static AUDIT_LOG: Lazy<Mutex<Vec<AuditEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));
static FLAGGED_GAMES: Lazy<Mutex<HashMap<u64, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static DISPUTE_ASSIGNMENTS: Lazy<Mutex<HashMap<u64, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn add_audit(action: &str, target: &str, result: &str) {
    let entry = AuditEntry {
        timestamp: now_secs(),
        actor: "admin".to_string(),
        action: action.to_string(),
        target: target.to_string(),
        result: result.to_string(),
    };
    if let Ok(mut log) = AUDIT_LOG.lock() {
        log.push(entry);
        if log.len() > 500 {
            log.remove(0);
        }
    }
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i32>,
    pub elo_min: Option<i32>,
    pub elo_max: Option<i32>,
    pub kyc: Option<String>,
    pub banned: Option<bool>,
}

#[derive(Deserialize)]
struct BanReq {
    reason: String,
    duration_days: Option<u32>,
}

#[derive(Deserialize)]
struct EloOverrideReq {
    new_elo: u32,
    reason: String,
}

#[derive(Deserialize)]
struct ForceResignReq {
    winner: String,
}

#[derive(Deserialize)]
struct FlagGameReq {
    reason: String,
}

#[derive(Deserialize)]
struct RefundReq {
    wallet: String,
    lamports: u64,
    reason: String,
    /// Second factor for this financially-irreversible action (checked against
    /// ADMIN_TOKEN in addition to the X-API-Key transport gate).
    #[serde(default)]
    admin_token: String,
}

#[derive(Deserialize)]
struct IpBanReq {
    ip: String,
    reason: String,
}

#[derive(Deserialize)]
struct WhitelistReq {
    wallet: String,
}

#[derive(Deserialize)]
struct AssignDisputeReq {
    reviewer: String,
}

#[derive(Deserialize)]
struct AuditLogQuery {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct FeeReportQuery {
    period: Option<String>,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        // Players
        .route("/admin/players", get(list_players))
        .route(
            "/admin/players/{wallet}/history",
            get(get_player_elo_history),
        )
        .route("/admin/players/{wallet}/ban", post(ban_player))
        .route("/admin/players/{wallet}/elo-override", post(elo_override))
        // Sessions / active
        .route("/admin/active-sessions", get(list_active_sessions))
        // Wallet balances
        .route("/admin/feepayer-balance", get(get_feepayer_balance))
        .route("/admin/wallet-balances", get(get_wallet_balances))
        // Anti-cheat
        .route("/admin/anti-cheat/reports", get(anti_cheat_reports))
        .route("/admin/anti-cheat/game/{game_id}/eval", get(get_game_eval))
        // Games
        .route("/admin/games/{game_id}/force-resign", post(force_resign))
        .route("/admin/games/{game_id}/flag", post(flag_game))
        // Audit
        .route("/admin/audit-log", get(get_audit_log))
        // Logs stream
        .route("/admin/logs/stream", get(logs_stream))
        // Treasury
        .route("/admin/treasury/payouts", get(treasury_payouts))
        .route("/admin/treasury/fee-report", get(treasury_fee_report))
        .route("/admin/treasury/refund", post(treasury_refund))
        // Tournament extras
        .route(
            "/admin/tournament/{id}/escrow-balance",
            get(tournament_escrow_balance),
        )
        .route(
            "/admin/tournament/{id}/fund-prize",
            post(fund_tournament_prize),
        )
        .route(
            "/admin/tournament/{id}/fill-bots",
            post(fill_tournament_bots),
        )
        // Tasks / infra
        .route("/admin/tasks/status", get(tasks_status))
        .route("/admin/db/stats", get(db_stats))
        .route("/admin/tls/expiry", get(tls_expiry))
        // Token rotation (authority-key rotation is a runbook, not an endpoint —
        // see deploy/SECRETS_ROTATION.md; a "rotate" button that only logs is a footgun)
        .route("/admin/auth/rotate-token", post(rotate_token))
        // Moderation
        .route("/admin/moderation/ip-ban", post(ip_ban))
        .route("/admin/moderation/ip-bans", get(list_ip_bans))
        .route("/admin/moderation/whitelist", post(whitelist_player))
        // Disputes
        .route("/admin/disputes/{game_id}/assign", post(assign_dispute))
}

// ── Handler implementations ───────────────────────────────────────────────────

async fn anti_cheat_reports(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Real data only. This used to prepend two hardcoded fake reports (game 1001,
    // 1045), which is fabricated data on a compliance/moderation surface. Report
    // only genuinely flagged games (from flag_game / FLAGGED_GAMES).
    let flagged = FLAGGED_GAMES.lock().map(|f| f.clone()).unwrap_or_default();
    let mut reports: Vec<serde_json::Value> = Vec::new();
    for (game_id, reason) in &flagged {
        reports.push(json!({
            "game_id": game_id,
            "white": "—",
            "black": "—",
            "suspect": "Unknown",
            "verdict": "Flag",
            "wager": "—",
            "score": 0.0,
            "reason": reason,
            "status": "Flagged",
            "created_at": now_secs()
        }));
    }
    let assignments = DISPUTE_ASSIGNMENTS
        .lock()
        .map(|d| d.clone())
        .unwrap_or_default();
    let reports_with_assignments: Vec<_> = reports
        .into_iter()
        .map(|mut r| {
            if let Some(game_id) = r["game_id"].as_u64() {
                if let Some(reviewer) = assignments.get(&game_id) {
                    r["assigned_to"] = json!(reviewer);
                }
            }
            r
        })
        .collect();
    Ok(Json(json!({ "reports": reports_with_assignments })))
}

async fn get_game_eval(Path(game_id): Path<u64>) -> Result<Json<serde_json::Value>, StatusCode> {
    // Mock centipawn eval series; real impl would run Stockfish via anticheat crate
    let evals: Vec<serde_json::Value> = (0..20).map(|i| {
        let cp = (i as f64 * 13.0 + (i as f64).sin() * 80.0) as i64 - 40;
        json!({ "move_number": i + 1, "centipawns": cp, "best_move_cp": cp + (i as i64 % 3) * 20 })
    }).collect();
    Ok(Json(json!({ "game_id": game_id, "evals": evals })))
}

async fn list_players(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(200);
    let players = state
        .store
        .list_players(limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let bans = PLAYER_BANS.lock().map(|b| b.clone()).unwrap_or_default();
    let elo_overrides = ELO_OVERRIDES.lock().map(|e| e.clone()).unwrap_or_default();

    let players_json: Vec<_> = players
        .into_iter()
        .filter_map(|(wallet, username, kyc_status)| {
            let is_banned = bans.contains_key(&wallet);
            if let Some(true) = query.banned {
                if !is_banned {
                    return None;
                }
            }
            if let Some(false_) = query.banned {
                if !false_ && is_banned {
                    return None;
                }
            }
            let elo = elo_overrides.get(&wallet).copied().unwrap_or(1200);
            if let Some(min) = query.elo_min {
                if (elo as i32) < min {
                    return None;
                }
            }
            if let Some(max) = query.elo_max {
                if (elo as i32) > max {
                    return None;
                }
            }
            if let Some(ref kyc_filter) = query.kyc {
                if &kyc_status != kyc_filter {
                    return None;
                }
            }
            Some(json!({
                "wallet": wallet,
                "username": username,
                "kyc_status": kyc_status,
                "elo": elo,
                "banned": is_banned,
                "ban_reason": bans.get(&wallet).map(|b| &b.reason),
            }))
        })
        .collect();

    Ok(Json(json!({ "players": players_json })))
}

async fn get_player_elo_history(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = GameRepository::new(state.store.pool());
    let games = repo
        .get_games_by_player(&wallet, 50)
        .await
        .unwrap_or_default();
    let mut elo = 1200i32;
    let history: Vec<_> = games
        .iter()
        .enumerate()
        .map(|(i, _g)| {
            let delta = if i % 3 == 0 {
                12
            } else if i % 3 == 1 {
                -8
            } else {
                0
            };
            elo += delta;
            json!({ "game_number": i + 1, "elo": elo })
        })
        .collect();
    Ok(Json(json!({ "wallet": wallet, "history": history })))
}

async fn ban_player(
    Path(wallet): Path<String>,
    Json(req): Json<BanReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let entry = BanEntry {
        wallet: wallet.clone(),
        reason: req.reason.clone(),
        duration_days: req.duration_days,
        banned_at: now_secs(),
    };
    if let Ok(mut bans) = PLAYER_BANS.lock() {
        bans.insert(wallet.clone(), entry);
    }
    add_audit("ban_player", &wallet, "ok");
    info!("[admin] Banned player {} reason={}", wallet, req.reason);
    Ok(Json(json!({ "ok": true, "wallet": wallet })))
}

async fn elo_override(
    Path(wallet): Path<String>,
    Json(req): Json<EloOverrideReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Ok(mut overrides) = ELO_OVERRIDES.lock() {
        overrides.insert(wallet.clone(), req.new_elo);
    }
    add_audit(
        "elo_override",
        &wallet,
        &format!("new_elo={} reason={}", req.new_elo, req.reason),
    );
    info!(
        "[admin] ELO override for {} → {} reason={}",
        wallet, req.new_elo, req.reason
    );
    Ok(Json(
        json!({ "ok": true, "wallet": wallet, "new_elo": req.new_elo }),
    ))
}

async fn list_active_sessions(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = GameRepository::new(state.store.pool());
    let sessions = repo
        .list_active_sessions()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "sessions": sessions })))
}

async fn get_feepayer_balance(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signer::Signer;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    let feepayer_pubkey = state.feepayer.next().pubkey();
    let balance = rpc
        .get_balance(&feepayer_pubkey)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sol = balance as f64 / 1_000_000_000.0;
    Ok(Json(json!({
        "balance_lamports": balance,
        "balance_sol": format!("{:.4} SOL", sol),
        "pubkey": feepayer_pubkey.to_string()
    })))
}

async fn get_wallet_balances(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signer::Signer;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    let get_bal = |pubkey: &solana_sdk::pubkey::Pubkey| -> Result<(u64, String), StatusCode> {
        let balance = rpc
            .get_balance(pubkey)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let sol = format!("{:.4} SOL", balance as f64 / 1_000_000_000.0);
        Ok((balance, sol))
    };

    let fp_pk = state.feepayer.next().pubkey();
    let vps_pk = state.vps_authority.pubkey();
    let kyc_pk = state.kyc_authority.pubkey();
    let treasury_pk = state.host_treasury_pubkey;

    let (fp_bal, fp_sol) = get_bal(&fp_pk)?;
    let (vps_bal, vps_sol) = get_bal(&vps_pk)?;
    let (kyc_bal, kyc_sol) = get_bal(&kyc_pk)?;
    let (treasury_bal, treasury_sol) = get_bal(&treasury_pk)?;

    Ok(Json(json!({
        "feepayer":   { "pubkey": fp_pk.to_string(),       "balance_lamports": fp_bal,       "balance_sol": fp_sol },
        "vps_signer": { "pubkey": vps_pk.to_string(),      "balance_lamports": vps_bal,      "balance_sol": vps_sol },
        "kyc_signer": { "pubkey": kyc_pk.to_string(),      "balance_lamports": kyc_bal,      "balance_sol": kyc_sol },
        "treasury":   { "pubkey": treasury_pk.to_string(), "balance_lamports": treasury_bal, "balance_sol": treasury_sol },
    })))
}

async fn force_resign(
    Path(game_id): Path<u64>,
    Json(req): Json<ForceResignReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Honest 501: there is no on-chain resign/timeout instruction builder in the
    // backend, so this cannot submit a real transaction. The stub previously
    // returned {ok:true, sig:"admin_force_resign_pending"}, which could mislead an
    // operator into thinking the game was settled. To force a disputed game's
    // outcome, use POST /admin/dispute/resolve (dispute_authority). A dedicated
    // admin force-resign path is Phase 5 on-chain work.
    add_audit(
        "force_resign_attempt",
        &format!("game_{}", game_id),
        &format!("winner={} (rejected: not implemented)", req.winner),
    );
    warn!(
        "[admin] force_resign game {} rejected — no on-chain resign builder; use /admin/dispute/resolve",
        game_id
    );
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "ok": false,
            "error": "not_implemented",
            "detail": "Forcing a game outcome on-chain is not wired up. Use POST /admin/dispute/resolve to resolve a disputed game (dispute_authority). A direct admin resign instruction requires Phase 5 on-chain work.",
            "game_id": game_id,
        })),
    )
}

async fn flag_game(
    Path(game_id): Path<u64>,
    Json(req): Json<FlagGameReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Ok(mut flags) = FLAGGED_GAMES.lock() {
        flags.insert(game_id, req.reason.clone());
    }
    add_audit("flag_game", &format!("game_{}", game_id), &req.reason);
    info!("[admin] Flagged game {} reason={}", game_id, req.reason);
    Ok(Json(json!({ "ok": true, "game_id": game_id })))
}

async fn get_audit_log(
    Query(q): Query<AuditLogQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = q.limit.unwrap_or(100);
    let log = AUDIT_LOG.lock().map(|l| l.clone()).unwrap_or_default();
    let entries: Vec<_> = log.into_iter().rev().take(limit).collect();
    Ok(Json(json!({ "entries": entries, "total": entries.len() })))
}

async fn logs_stream() -> Result<Json<serde_json::Value>, StatusCode> {
    // Honest empty stream: real log streaming needs either `axum::response::Sse`
    // fed by a tokio broadcast channel wired into the tracing subscriber, or the
    // panel tailing journald over SSH (it already has an SSH terminal). The stub
    // fabricated "backend started / metrics polled / health check OK" lines every
    // poll, which is noise that masks the absence of real logs. Return nothing
    // until a real source is wired.
    Ok(Json(json!({
        "lines": [],
        "note": "in-app log streaming not wired; tail journald via the Hetzner SSH panel or `journalctl -u xfchess-backend -f`",
    })))
}

async fn treasury_payouts(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = GameRepository::new(state.store.pool());
    let games = repo.list_games(Some(50), None).await.unwrap_or_default();
    let payouts: Vec<_> = games
        .iter()
        .filter_map(|g| {
            let stake = g.stake_amount;
            if stake < 0.000001 {
                return None;
            }
            Some(json!({
                "game_id": g.id,
                "winner": g.winner,
                "amount_sol": stake,
                "tx_sig": g.finalize_sig.as_deref().unwrap_or("—"),
                "settled_at": g.end_time,
            }))
        })
        .collect();
    Ok(Json(json!({ "payouts": payouts })))
}

async fn treasury_fee_report(
    State(state): State<AppState>,
    Query(q): Query<FeeReportQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let period = q.period.unwrap_or_else(|| "week".to_string());
    let repo = GameRepository::new(state.store.pool());
    let games = repo.list_games(Some(200), None).await.unwrap_or_default();
    let total_fees: i64 = games.iter().map(|g| g.fee_lamports).sum();
    let total_stake: f64 = games.iter().map(|g| g.stake_amount).sum();
    Ok(Json(json!({
        "total_fee_sol": total_fees as f64 / 1e9,
        "total_fee_lamports": total_fees,
        "total_wagered_sol": total_stake,
        "game_count": games.len(),
        "period": period,
    })))
}

/// Withdraw `lamports` from the platform treasury vault to `wallet`, signed by
/// `treasury_authority`. Double-gated: the X-API-Key transport gate PLUS an
/// ADMIN_TOKEN second factor in the body (this is a financially-irreversible
/// money path). Requires the on-chain `withdraw_treasury` instruction to be
/// deployed and `TREASURY_AUTHORITY_KEY` set to the treasury_authority keypair.
async fn treasury_refund(
    State(state): State<AppState>,
    Json(req): Json<RefundReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    use crate::infrastructure::auth_middleware::constant_time_eq;
    use crate::signing::solana::{make_rpc, sign_and_submit, withdraw_treasury_ix};
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::Signer;
    use std::str::FromStr;

    // Second factor — constant-time; reject if ADMIN_TOKEN is unset or mismatched.
    let expected = state.config.admin_token.clone().unwrap_or_default();
    if expected.is_empty() || !constant_time_eq(&req.admin_token, &expected) {
        warn!("[admin] treasury_refund rejected — bad/missing ADMIN_TOKEN second factor");
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "ok": false, "error": "bad_admin_token" })),
        );
    }

    if req.lamports == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "amount_must_be_positive" })),
        );
    }
    let destination = match Pubkey::from_str(&req.wallet) {
        Ok(pk) => pk,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": "invalid_wallet_pubkey" })),
            );
        }
    };

    let program_id = state.program_id;
    let rpc_url = state.config.solana_rpc_url.clone();
    let authority = state.treasury_authority.clone();
    let amount = req.lamports;

    let result = tokio::task::spawn_blocking(move || {
        let rpc = make_rpc(&rpc_url);
        let ix = withdraw_treasury_ix(&program_id, &authority.pubkey(), &destination, amount);
        sign_and_submit(&rpc, &authority, &[ix])
    })
    .await;

    match result {
        Ok(Ok(sig)) => {
            add_audit(
                "treasury_refund",
                &req.wallet,
                &format!("{} lamports reason={} sig={}", req.lamports, req.reason, sig),
            );
            info!(
                "[admin] treasury_refund {} lamports -> {} ({}) sig {}",
                req.lamports, req.wallet, req.reason, sig
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "wallet": req.wallet,
                    "lamports": req.lamports,
                    "signature": sig.to_string(),
                })),
            )
        }
        Ok(Err(e)) => {
            add_audit(
                "treasury_refund_failed",
                &req.wallet,
                &format!("{} lamports reason={} err={}", req.lamports, req.reason, e),
            );
            error!("[admin] treasury_refund on-chain submit failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "onchain_submit_failed", "detail": e.to_string() })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": "task_join_failed", "detail": e.to_string() })),
        ),
    }
}

async fn tournament_escrow_balance(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let program_id = state.program_id;
    // Tournament escrow seed is "t_escrow" ("escrow" is the per-game wager seed).
    let seeds = &[b"t_escrow", &id.to_le_bytes()[..]];
    let (escrow_pda, _bump) = solana_sdk::pubkey::Pubkey::find_program_address(seeds, &program_id);
    let rpc = state.solana_rpc.clone();
    let balance = tokio::task::spawn_blocking(move || rpc.get_balance(&escrow_pda).unwrap_or(0))
        .await
        .unwrap_or(0);
    Ok(Json(json!({
        "tournament_id": id,
        "escrow_pda": escrow_pda.to_string(),
        "balance_lamports": balance,
        "balance_sol": balance as f64 / 1e9,
    })))
}

#[derive(Deserialize)]
struct FundPrizeRequest {
    amount_lamports: u64,
}

/// Locks the guaranteed SOL prize for a tournament in its escrow PDA.
/// Must be called after initialization but BEFORE the first registration —
/// the program rejects registrations on paid tournaments until this runs,
/// and rejects funding once anyone has registered (the guarantee is immutable).
/// Funds are drawn from the server's vps_authority wallet.
async fn fund_tournament_prize(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<FundPrizeRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::signing::solana::{fund_sol_prize_ix, make_rpc, sign_and_submit};
    use solana_sdk::signature::Signer;

    if req.amount_lamports == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let program_id = state.program_id;
    let rpc_url = state.config.solana_rpc_url.clone();
    let authority = state.vps_authority.clone();
    let amount = req.amount_lamports;

    let sig = tokio::task::spawn_blocking(move || {
        let rpc = make_rpc(&rpc_url);
        let ix = fund_sol_prize_ix(&program_id, id, &authority.pubkey(), amount);
        sign_and_submit(&rpc, &authority, &[ix])
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        tracing::error!("[admin] fund_sol_prize failed for tournament {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    add_audit(
        "fund_prize",
        &format!("tournament:{}", id),
        &format!("{} lamports", amount),
    );
    info!(
        "[admin] Guaranteed prize of {} lamports locked for tournament {} ({})",
        amount, id, sig
    );
    Ok(Json(json!({
        "ok": true,
        "tournament_id": id,
        "amount_lamports": amount,
        "signature": sig.to_string(),
    })))
}

async fn tasks_status() -> Result<Json<serde_json::Value>, StatusCode> {
    // Honest placeholder: the background workers do not yet publish their last-tick
    // timestamps to a shared registry, so we cannot report real liveness. The stub
    // used to fabricate "ok, ticked 30s ago" for every worker, which hides an actually
    // dead worker. Report unknown until the workers are instrumented (e.g. an
    // AtomicU64 last_tick per worker surfaced here or via /metrics).
    let unknown = json!({ "last_tick": null, "status": "not_instrumented" });
    Ok(Json(json!({
        "tournament_scheduler": unknown,
        "settlement_worker":    unknown,
        "prize_distributor":    unknown,
        "note": "worker last-tick instrumentation not yet wired; see /metrics for scrape counters",
    })))
}

async fn db_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let sessions_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    let games_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_history")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    let users_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let db_path = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sessions.db".to_string())
        .replace("sqlite://", "");
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(Json(json!({
        "sessions_rows": sessions_rows,
        "games_rows": games_rows,
        "users_rows": users_rows,
        "db_bytes": db_bytes,
        "db_mb": db_bytes as f64 / 1_048_576.0,
    })))
}

async fn tls_expiry() -> Result<Json<serde_json::Value>, StatusCode> {
    let cert_path = "/etc/letsencrypt/live/xfchess.gg/cert.pem";
    if std::path::Path::new(cert_path).exists() {
        // Real impl: parse PEM, extract NotAfter with openssl crate
        Ok(Json(json!([{
            "domain": "xfchess.gg",
            "cert_path": cert_path,
            "status": "found",
            "days_remaining": null,
            "note": "parse with openssl crate for exact expiry"
        }])))
    } else {
        Ok(Json(json!([{
            "domain": "xfchess.gg",
            "status": "no_cert",
            "days_remaining": null,
            "note": "TLS not yet configured on this node"
        }])))
    }
}

async fn rotate_token() -> Result<Json<serde_json::Value>, StatusCode> {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    let new_token = format!("xf_admin_{}", hex::encode(bytes));
    add_audit("rotate_token", "admin_token", "rotated");
    info!("[admin] Admin token rotated");
    Ok(Json(
        json!({ "ok": true, "new_token": new_token, "note": "Update ADMIN_API_KEY in .env and restart the backend." }),
    ))
}

async fn ip_ban(Json(req): Json<IpBanReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let entry = IpBanEntry {
        ip: req.ip.clone(),
        reason: req.reason.clone(),
        banned_at: now_secs(),
    };
    if let Ok(mut bans) = IP_BANS.lock() {
        bans.push(entry);
    }
    add_audit("ip_ban", &req.ip, &req.reason);
    info!("[admin] IP banned: {} reason={}", req.ip, req.reason);
    Ok(Json(json!({ "ok": true, "ip": req.ip })))
}

async fn list_ip_bans() -> Result<Json<serde_json::Value>, StatusCode> {
    let bans = IP_BANS.lock().map(|b| b.clone()).unwrap_or_default();
    Ok(Json(json!({ "bans": bans })))
}

async fn whitelist_player(
    Json(req): Json<WhitelistReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Ok(mut wl) = WHITELIST.lock() {
        if !wl.contains(&req.wallet) {
            wl.push(req.wallet.clone());
        }
    }
    add_audit("whitelist", &req.wallet, "added");
    Ok(Json(json!({ "ok": true, "wallet": req.wallet })))
}

async fn assign_dispute(
    Path(game_id): Path<u64>,
    Json(req): Json<AssignDisputeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Ok(mut assigns) = DISPUTE_ASSIGNMENTS.lock() {
        assigns.insert(game_id, req.reviewer.clone());
    }
    add_audit(
        "assign_dispute",
        &format!("game_{}", game_id),
        &req.reviewer,
    );
    Ok(Json(
        json!({ "ok": true, "game_id": game_id, "reviewer": req.reviewer }),
    ))
}

#[derive(Deserialize)]
struct FillBotsReq {
    /// How many bot slots to fill (default: fills to max_players)
    count: Option<u16>,
    /// Base ELO assigned to bots (default: 1200)
    elo: Option<u32>,
}

/// POST /admin/tournament/:id/fill-bots — fills remaining slots with fake wallets and starts the
/// bracket. Dev/test only; lets you simulate a full tournament without real players.
async fn fill_tournament_bots(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<FillBotsReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    let max = tournament.max_players as usize;
    let current = tournament.players.len();
    let bot_elo = req.elo.unwrap_or(1200);
    let fill_count = req
        .count
        .map(|c| (c as usize).min(max.saturating_sub(current)))
        .unwrap_or(max.saturating_sub(current));

    if fill_count == 0 {
        return Ok(Json(
            json!({ "ok": true, "added": 0, "message": "tournament already full" }),
        ));
    }

    let bots: Vec<String> = (0..fill_count)
        .map(|i| format!("Bot{:04}_{:08x}", i, id))
        .collect();

    store
        .update(id, |t| {
            for bot in &bots {
                t.players.push(bot.clone());
                t.player_elos.push(bot_elo);
                t.prize_pool += t.entry_fee_lamports.saturating_sub(t.platform_fee_lamports);
            }
        })
        .await;

    // Generate bracket and start
    store.generate_bracket(id).await;
    match store.start_tournament(id).await {
        Ok(()) => {
            add_audit(
                "fill_bots",
                &format!("tournament_{}", id),
                &format!("added {} bots", fill_count),
            );
            info!(
                "[admin] Filled tournament {} with {} bots, started bracket",
                id, fill_count
            );
            Ok(Json(json!({
                "ok": true,
                "tournament_id": id,
                "added": fill_count,
                "total_players": current + fill_count,
                "bots": bots,
            })))
        }
        Err(e) => {
            tracing::error!("[admin] fill-bots: start_tournament failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
