//! The admin API surface: player moderation, anti-cheat review, wallet/treasury
//! balances and payouts, tournament escrow inspection, and operational status
//! endpoints. Every route here is gated by the admin auth middleware upstream
//! (not per-handler); `treasury_refund` additionally requires the
//! `ADMIN_TOKEN` second factor since it moves real funds. IP bans, ELO
//! overrides, and the audit log are in-memory only (`Lazy<Mutex<...>>`) —
//! they reset on backend restart, unlike everything routed through
//! `state.store` (SQLite-backed). See `admin_routes()` for the full route map.

use crate::db::repository::GameRepository;
async fn tournament_transactions(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "tournament_id": id,
        "transactions": state.tournament_store.transactions(id).await,
    })))
}
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

static ELO_OVERRIDES: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static IP_BANS: Lazy<Mutex<Vec<IpBanEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));
static AUDIT_LOG: Lazy<Mutex<Vec<AuditEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Records an admin action. `actor` is resolved from the API key that
/// authenticated the current request (see `auth_middleware::current_admin_actor`).
/// Always updates the in-memory tail (fast path for the live view); when
/// `pool` is given also persists to `admin_audit_log` **before returning** so
/// the entry survives a restart. Awaited (not fire-and-forget) deliberately —
/// an audit log that can silently lose an entry to a crash between "response
/// sent" and "write landed" defeats its own purpose. `pool` is only absent in
/// the handful of handlers that predate `AppState` threading (elo override,
/// IP ban, token rotate) — those stay memory-only until they're refactored to
/// take `State<AppState>`.
async fn add_audit(pool: Option<sqlx::SqlitePool>, action: &str, target: &str, result: &str) {
    let actor = crate::infrastructure::auth_middleware::current_admin_actor();
    let ts = now_secs();
    let entry = AuditEntry {
        timestamp: ts,
        actor: actor.clone(),
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
    if let Some(pool) = pool {
        if let Err(e) = sqlx::query(
            "INSERT INTO admin_audit_log (ts, actor, action, target, result, method, path, status) \
             VALUES (?, ?, ?, ?, ?, '', '', NULL)",
        )
        .bind(ts as i64)
        .bind(&actor)
        .bind(action)
        .bind(target)
        .bind(result)
        .execute(&pool)
        .await
        {
            error!("[admin] failed to persist audit entry ({action} {target}): {e}");
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
struct AssignDisputeReq {
    reviewer: String,
}

#[derive(Deserialize)]
struct AuditLogQuery {
    limit: Option<usize>,
    tournament_id: Option<u64>,
}

#[derive(Deserialize)]
struct FeeReportQuery {
    period: Option<String>,
}

#[derive(Deserialize)]
struct TournamentCostRequest {
    max_players: u16,
    format: String,
    #[serde(default)]
    average_moves: u32,
}

#[derive(Deserialize)]
struct AffordabilityQuery {
    max_players: Option<u16>,
    format: Option<String>,
    average_moves: Option<u32>,
}

pub(crate) fn tournament_cost_estimate(
    max_players: u16,
    format: &str,
    average_moves: u32,
) -> serde_json::Value {
    let games = max_players.saturating_sub(1) as u64;
    let shards = max_players.div_ceil(64) as u64;
    let shard_rent = shards * 34_100_000;
    let match_rent = games * 2_170_000;
    let game_rent = games * 3_400_000;
    let tx_count = if format.eq_ignore_ascii_case("swiss") {
        u64::from(max_players) * 3 + games
    } else {
        u64::from(max_players) * 2 + games * 3
    };
    let base_fees = tx_count * 5_000;
    let priority_fees = tx_count * 200_000 * 10_000 / 1_000_000;
    let er_session = games * 300_000;
    let er_moves = games * u64::from(average_moves) * 5_000;
    let gross =
        shard_rent + match_rent + game_rent + base_fees + priority_fees + er_session + er_moves;
    let refunds = shard_rent + match_rent + game_rent;
    serde_json::json!({
        "gross_lamports": gross,
        "net_lamports": gross.saturating_sub(refunds),
        "working_capital_lamports": gross,
        "per_player_lamports": if max_players == 0 { 0 } else { gross / u64::from(max_players) },
        "rent_lamports": shard_rent + match_rent + game_rent,
        "er_lamports": er_session + er_moves,
        "er_cost_confidence": "modeled",
        "assumptions": {
            "average_moves": average_moves,
            "format": format,
            "rent_refunds_modeled": true
        }
    })
}

async fn predict_tournament_cost(
    State(_state): State<AppState>,
    Json(req): Json<TournamentCostRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(tournament_cost_estimate(
        req.max_players,
        &req.format,
        req.average_moves,
    )))
}

async fn operator_affordability(
    State(state): State<AppState>,
    Query(q): Query<AffordabilityQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signature::Signer;
    let rpc = state.solana_rpc.clone();
    let wallet = state.vps_authority.pubkey();
    let balance = tokio::task::spawn_blocking(move || rpc.get_balance(&wallet).unwrap_or(0))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // max_players defaults to the largest bracket size so an unparameterised
    // call reports the worst case rather than silently claiming affordability
    // for a size nobody asked about.
    let max_players = q.max_players.unwrap_or(256);
    let format = q.format.as_deref().unwrap_or("SingleElimination");
    let average_moves = q.average_moves.unwrap_or(40);
    let estimate = tournament_cost_estimate(max_players, format, average_moves);
    let required = estimate
        .get("working_capital_lamports")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);

    Ok(Json(json!({
        "wallet": wallet.to_string(),
        "balance_lamports": balance,
        "balance_sol": balance as f64 / 1e9,
        "predicted_for": { "max_players": max_players, "format": format, "average_moves": average_moves },
        "required_working_capital_lamports": required,
        "covers_prediction": balance >= required
    })))
}

#[derive(Deserialize)]
struct SaveTemplateReq {
    name: String,
    data: serde_json::Value,
}

/// GET /admin/tournament-templates — lists all saved templates.
async fn list_templates(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT name, data_json, created_at, updated_at FROM tournament_templates ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("[admin] failed to list templates: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let templates: Vec<_> = rows
        .into_iter()
        .map(|(name, data_json, created_at, updated_at)| {
            json!({
                "name": name,
                "data": serde_json::from_str::<serde_json::Value>(&data_json).unwrap_or(json!(null)),
                "created_at": created_at,
                "updated_at": updated_at,
            })
        })
        .collect();
    Ok(Json(json!({ "templates": templates })))
}

/// POST /admin/tournament-templates — creates or overwrites a named template.
async fn save_template(
    State(state): State<AppState>,
    Json(req): Json<SaveTemplateReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let pool = state.store.pool();
    let now = now_secs() as i64;
    let data_json = serde_json::to_string(&req.data).map_err(|_| StatusCode::BAD_REQUEST)?;
    sqlx::query(
        "INSERT INTO tournament_templates (name, data_json, created_at, updated_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT(name) DO UPDATE SET data_json = excluded.data_json, updated_at = excluded.updated_at",
    )
    .bind(&req.name)
    .bind(&data_json)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("[admin] failed to save template {}: {}", req.name, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    add_audit(Some(pool), "save_template", &req.name, "ok").await;
    info!("[admin] Saved tournament template '{}'", req.name);
    Ok(Json(json!({ "ok": true, "name": req.name })))
}

/// DELETE /admin/tournament-templates/:name
async fn delete_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let result = sqlx::query("DELETE FROM tournament_templates WHERE name = ?")
        .bind(&name)
        .execute(&pool)
        .await
        .map_err(|e| {
            error!("[admin] failed to delete template {}: {}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    add_audit(Some(pool), "delete_template", &name, "ok").await;
    info!("[admin] Deleted tournament template '{}'", name);
    Ok(Json(json!({ "ok": true, "name": name })))
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Builds the full `/admin/*` route tree (players, sessions, wallet balances,
/// anti-cheat, games, audit log, treasury, tournament extras, tasks/infra
/// status, moderation, disputes). Nest this on an app already behind admin
/// auth middleware.
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
        // Tournament templates
        .route(
            "/admin/tournament-templates",
            get(list_templates).post(save_template),
        )
        .route(
            "/admin/tournament-templates/{name}",
            axum::routing::delete(delete_template),
        )
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
            "/admin/tournament/{id}/transactions",
            get(tournament_transactions),
        )
        .route(
            "/admin/tournament/{id}/fund-prize",
            post(fund_tournament_prize),
        )
        .route(
            "/admin/tournament/predict-cost",
            post(predict_tournament_cost),
        )
        .route("/admin/operator-affordability", get(operator_affordability))
        .route(
            "/admin/tournament/{id}/fill-bots",
            post(fill_tournament_bots),
        )
        // Tasks / infra
        .route("/admin/tasks/status", get(tasks_status))
        .route("/admin/metrics/summary", get(metrics_summary))
        .route("/admin/db/stats", get(db_stats))
        .route("/admin/tls/expiry", get(tls_expiry))
        // Token rotation (authority-key rotation is a runbook, not an endpoint —
        // see ops/SECRETS_ROTATION.md; a "rotate" button that only logs is a footgun)
        .route("/admin/auth/rotate-token", post(rotate_token))
        // Moderation
        .route("/admin/moderation/ip-ban", post(ip_ban))
        .route("/admin/moderation/ip-bans", get(list_ip_bans))
        // Disputes
        .route("/admin/disputes/{game_id}/assign", post(assign_dispute))
}

// ── Handler implementations ───────────────────────────────────────────────────

async fn anti_cheat_reports(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Real data only. This used to prepend two hardcoded fake reports (game 1001,
    // 1045), which is fabricated data on a compliance/moderation surface. Report
    // only genuinely flagged games, persisted in flagged_games (migration 024).
    let flagged_repo = crate::db::repository::FlaggedGameRepository::new(state.store.pool());
    let flagged = flagged_repo.list().await.unwrap_or_default();
    let reports: Vec<serde_json::Value> = flagged
        .into_iter()
        .map(|f| {
            json!({
                "game_id": f.game_id,
                "white": "—",
                "black": "—",
                "suspect": "Unknown",
                "verdict": "Flag",
                "wager": "—",
                "score": 0.0,
                "reason": f.reason,
                "status": "Flagged",
                "created_at": f.flagged_at,
                "assigned_to": f.assigned_to,
            })
        })
        .collect();
    Ok(Json(json!({ "reports": reports })))
}

#[derive(sqlx::FromRow)]
struct AnticheatVerdictRow {
    white_pubkey: String,
    black_pubkey: String,
    white_verdict: String,
    black_verdict: String,
    white_score: f64,
    black_score: f64,
    white_signals: String,
    black_signals: String,
    analysed_at: i64,
}

/// GET /admin/anti-cheat/game/{id}/eval — real Stockfish-derived verdict for a
/// game, from `anticheat_verdicts` (populated by `tasks/anticheat_worker.rs`
/// after finalization). Aggregate per-game only — no move-by-move eval curve
/// is stored today, so this reports the verdict/score/signals the worker
/// actually computed rather than fabricating a per-move series.
async fn get_game_eval(
    Path(game_id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let row = sqlx::query_as::<_, AnticheatVerdictRow>(
        "SELECT white_pubkey, black_pubkey, white_verdict, black_verdict, \
         white_score, black_score, white_signals, black_signals, analysed_at \
         FROM anticheat_verdicts WHERE game_id = ? ORDER BY analysed_at DESC LIMIT 1",
    )
    .bind(game_id.to_string())
    .fetch_optional(&state.store.pool())
    .await
    .map_err(|e| {
        error!("[admin] eval query failed for game {}: {}", game_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        None => Ok(Json(json!({ "game_id": game_id, "analysed": false }))),
        Some(r) => {
            let white_signals: serde_json::Value =
                serde_json::from_str(&r.white_signals).unwrap_or(json!({}));
            let black_signals: serde_json::Value =
                serde_json::from_str(&r.black_signals).unwrap_or(json!({}));
            Ok(Json(json!({
                "game_id": game_id,
                "analysed": true,
                "white": { "pubkey": r.white_pubkey, "verdict": r.white_verdict, "score": r.white_score, "signals": white_signals },
                "black": { "pubkey": r.black_pubkey, "verdict": r.black_verdict, "score": r.black_score, "signals": black_signals },
                "analysed_at": r.analysed_at,
            })))
        }
    }
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

    let ban_repo = crate::db::repository::BanRepository::new(state.store.pool());
    let bans: HashMap<String, crate::db::repository::BanRecord> = ban_repo
        .list()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.wallet.clone(), b))
        .collect();
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

// No per-game ELO snapshot is recorded anywhere (on-chain PlayerProfile only
// keeps the current rating, not a history) — this used to fabricate a
// +12/-8/0 cycling delta series. Real per-game outcomes are all that's
// actually available, so that's what this reports instead of inventing an
// ELO curve.
async fn get_player_elo_history(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = GameRepository::new(state.store.pool());
    let games = repo
        .get_games_by_player(&wallet, 50)
        .await
        .unwrap_or_default();
    let history: Vec<_> = games
        .iter()
        .map(|g| {
            let result = match &g.winner {
                None if g.status != "completed" => "in_progress",
                None => "draw",
                Some(w) if w == &wallet => "win",
                Some(_) => "loss",
            };
            json!({
                "game_id": g.id,
                "result": result,
                "stake_amount": g.stake_amount,
                "ended_at": g.end_time,
            })
        })
        .collect();
    Ok(Json(
        json!({ "wallet": wallet, "history": history, "elo_history_available": false }),
    ))
}

/// Persists a ban for `wallet` (SQLite-backed via `BanRepository`, survives restart).
async fn ban_player(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
    Json(req): Json<BanReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let bans = crate::db::repository::BanRepository::new(state.store.pool());
    bans.ban(&wallet, &req.reason, req.duration_days)
        .await
        .map_err(|e| {
            error!("[admin] Failed to persist ban for {}: {}", wallet, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    add_audit(Some(state.store.pool()), "ban_player", &wallet, "ok").await;
    info!("[admin] Banned player {} reason={}", wallet, req.reason);
    Ok(Json(json!({ "ok": true, "wallet": wallet })))
}

/// Sets a display-only ELO override for `wallet` in the in-memory
/// `ELO_OVERRIDES` map (read back by `list_players`). Does not touch the
/// on-chain `elo_rating` and does not survive a backend restart.
async fn elo_override(
    Path(wallet): Path<String>,
    Json(req): Json<EloOverrideReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Ok(mut overrides) = ELO_OVERRIDES.lock() {
        overrides.insert(wallet.clone(), req.new_elo);
    }
    add_audit(
        None,
        "elo_override",
        &wallet,
        &format!("new_elo={} reason={}", req.new_elo, req.reason),
    )
    .await;
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
    let treasury_pk = state.tournament_fee_recipient;

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
        None,
        "force_resign_attempt",
        &format!("game_{}", game_id),
        &format!("winner={} (rejected: not implemented)", req.winner),
    )
    .await;
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
    State(state): State<AppState>,
    Path(game_id): Path<u64>,
    Json(req): Json<FlagGameReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let flagged_repo = crate::db::repository::FlaggedGameRepository::new(state.store.pool());
    flagged_repo
        .flag(game_id as i64, &req.reason)
        .await
        .map_err(|e| {
            error!("[admin] Failed to persist flag for game {}: {}", game_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    add_audit(
        Some(state.store.pool()),
        "flag_game",
        &format!("game_{}", game_id),
        &req.reason,
    )
    .await;
    info!("[admin] Flagged game {} reason={}", game_id, req.reason);
    Ok(Json(json!({ "ok": true, "game_id": game_id })))
}

#[derive(sqlx::FromRow, Serialize)]
struct PersistedAuditRow {
    timestamp: i64,
    actor: String,
    action: String,
    target: String,
    result: String,
}

/// Reads the persistent `admin_audit_log` table (survives a backend restart,
/// unlike the old in-memory-only `AUDIT_LOG` Vec this replaced as the primary
/// source). Falls back to the in-memory tail only if the DB read fails, so a
/// transient SQLite hiccup doesn't blank the panel's audit view.
async fn get_audit_log(
    State(state): State<AppState>,
    Query(q): Query<AuditLogQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = q.limit.unwrap_or(100) as i64;
    let pool = state.store.pool();

    let rows: Result<Vec<PersistedAuditRow>, sqlx::Error> = if let Some(id) = q.tournament_id {
        let target_a = format!("tournament:{}", id);
        let target_b = format!("tournament_{}", id);
        let path_needle = format!("/tournament/{}/", id);
        sqlx::query_as(
            "SELECT ts as timestamp, actor, action, target, result \
             FROM admin_audit_log \
             WHERE target = ?1 OR target = ?2 OR path LIKE '%' || ?3 || '%' \
             ORDER BY id DESC LIMIT ?4",
        )
        .bind(&target_a)
        .bind(&target_b)
        .bind(&path_needle)
        .bind(limit)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT ts as timestamp, actor, action, target, result \
             FROM admin_audit_log ORDER BY id DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&pool)
        .await
    };

    let entries = match rows {
        Ok(rows) => serde_json::to_value(rows).unwrap_or(json!([])),
        Err(e) => {
            error!(
                "[admin] audit log DB read failed, falling back to in-memory tail: {}",
                e
            );
            let log = AUDIT_LOG.lock().map(|l| l.clone()).unwrap_or_default();
            let entries: Vec<_> = log
                .into_iter()
                .rev()
                .filter(|entry| {
                    q.tournament_id
                        .map(|id| {
                            entry.target == format!("tournament:{}", id)
                                || entry.target == format!("tournament_{}", id)
                        })
                        .unwrap_or(true)
                })
                .take(limit as usize)
                .collect();
            serde_json::to_value(entries).unwrap_or(json!([]))
        }
    };
    let total = entries.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(Json(json!({ "entries": entries, "total": total })))
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

/// Records a request to withdraw `lamports` from the platform treasury vault
/// to `wallet`, and hands back the exact `treasury_signer` command an
/// operator must run on a separate, minimally-networked host to actually
/// execute it. This process **never holds or uses the treasury signing
/// key** — see `bin/treasury_signer.rs`'s module doc: a compromise of this
/// always-on, internet-facing host must not also be able to drain the
/// treasury. Double-gated: the X-API-Key transport gate PLUS an ADMIN_TOKEN
/// second factor in the body (this is a financially-irreversible money
/// path) — the only admin route with this extra gate, since it's the only
/// one that moves real funds outside normal settlement. The audit log entry
/// is written here (request time), and `treasury_signer` itself logs the
/// actual signature on submission — the two are correlated by wallet +
/// lamports + reason in the audit trail.
async fn treasury_refund(
    State(state): State<AppState>,
    Json(req): Json<RefundReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    use crate::infrastructure::auth_middleware::constant_time_eq;
    use solana_sdk::pubkey::Pubkey;
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
    if Pubkey::from_str(&req.wallet).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "invalid_wallet_pubkey" })),
        );
    }

    add_audit(
        Some(state.store.pool()),
        "treasury_refund_requested",
        &req.wallet,
        &format!(
            "{} lamports reason={} — awaiting manual execution via treasury_signer",
            req.lamports, req.reason
        ),
    )
    .await;
    info!(
        "[admin] treasury_refund requested: {} lamports -> {} ({}); operator must run treasury_signer",
        req.lamports, req.wallet, req.reason
    );

    // Structured arguments, not a shell line. The previous version interpolated
    // `state.config.solana_rpc_url` raw — that URL carries the Triton x-token in
    // its path, the very secret `solana::redact_url` exists to keep out of logs,
    // and this handler put it in an HTTP response body and the audit trail. It
    // also spliced the operator-supplied `reason` into the command with nothing
    // but surrounding double quotes, so a reason containing `"; …` produced a
    // line that ran attacker-chosen text the moment it was pasted into the
    // isolated host — the one machine in this design that holds the treasury key.
    //
    // The operator supplies the RPC URL and key from that host's own
    // environment; neither belongs in a response from this process.
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "status": "awaiting_manual_execution",
            "wallet": req.wallet,
            "lamports": req.lamports,
            "reason": req.reason,
            "run_on_isolated_host": {
                "binary": "treasury_signer",
                "args": [req.wallet, req.lamports.to_string(), req.reason],
                "env_required": ["TREASURY_AUTHORITY_KEY", "SOLANA_RPC_URL"],
                "program_id": state.program_id.to_string(),
                "rpc_cluster": crate::signing::solana::redact_url(&state.config.solana_rpc_url),
                "note": "Set TREASURY_AUTHORITY_KEY and SOLANA_RPC_URL from the signing host's own \
                         environment. Pass the args exactly as listed — do not build a shell \
                         string from them.",
            },
        })),
    )
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

    let submit_started = std::time::Instant::now();
    state.metrics.record_transaction_submitted("solana");
    let sig = tokio::task::spawn_blocking(move || {
        let rpc = make_rpc(&rpc_url);
        let ix = fund_sol_prize_ix(&program_id, id, &authority.pubkey(), amount);
        sign_and_submit(&rpc, &authority, &[ix])
    })
    .await
    .map_err(|_| {
        state
            .metrics
            .record_transaction_failed("solana", "TaskJoinError");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        let category = crate::signing::solana::classify_error_str(&e.to_string()).to_string();
        state.metrics.record_transaction_failed("solana", &category);
        tracing::error!("[admin] fund_sol_prize failed for tournament {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state
        .metrics
        .record_transaction_confirmed("solana", submit_started.elapsed().as_millis() as f64);

    state
        .tournament_store
        .record_transaction(crate::signing::storage::tournament::TournamentTransaction {
            tournament_id: id,
            signature: sig.to_string(),
            operation: "fund_prize".to_string(),
            status: "confirmed".to_string(),
            retry_count: 0,
            last_error: None,
            next_retry_at: None,
            created_at: chrono::Utc::now().timestamp(),
        })
        .await;

    // Mirror the confirmed on-chain lock into the store's prize_pool field so
    // GET /tournaments (list_tournaments) shows a real, chain-confirmed
    // figure instead of the stale local accumulator it used to be (removed
    // from join/fill-bots — see F2 in tournament-production-readiness-plan.md).
    // Cumulative: fund_sol_prize can be called more than once per tournament.
    state
        .tournament_store
        .update(id, |t| {
            t.prize_pool = t.prize_pool.saturating_add(amount);
        })
        .await;

    add_audit(
        Some(state.store.pool()),
        "fund_prize",
        &format!("tournament:{}", id),
        &format!("{} lamports", amount),
    )
    .await;
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

/// Reports real per-worker liveness from the `*_LAST_TICK_UNIX` gauges each
/// background loop stamps on every pass (`telemetry::worker_metrics`). A
/// worker whose last tick is more than 2x its own poll interval old is
/// flagged `stale` — most likely panicked mid-loop and silently stopped
/// (the process itself is still up, `/health` would still say ok).
async fn tasks_status() -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::telemetry::worker_metrics::{
        PRIZE_DISTRIBUTOR_LAST_TICK_UNIX, SETTLEMENT_LAST_TICK_UNIX,
        TOURNAMENT_SCHEDULER_LAST_TICK_UNIX,
    };
    use std::sync::atomic::Ordering;

    let now = now_secs();
    let worker_status = |last_tick_unix: u64, expected_interval_secs: u64| {
        if last_tick_unix == 0 {
            return json!({ "last_tick": null, "status": "not_yet_ticked" });
        }
        let age = now.saturating_sub(last_tick_unix);
        let status = if age > expected_interval_secs * 2 {
            "stale"
        } else {
            "ok"
        };
        json!({ "last_tick": last_tick_unix, "status": status, "age_seconds": age })
    };

    Ok(Json(json!({
        "settlement_worker": worker_status(SETTLEMENT_LAST_TICK_UNIX.load(Ordering::Relaxed), 30),
        "tournament_scheduler": worker_status(
            TOURNAMENT_SCHEDULER_LAST_TICK_UNIX.load(Ordering::Relaxed),
            30
        ),
        "prize_distributor": worker_status(
            PRIZE_DISTRIBUTOR_LAST_TICK_UNIX.load(Ordering::Relaxed),
            60
        ),
    })))
}

/// Real numbers for the admin panel's KPI tiles — presence, confirmed
/// transactions, ER staleness/subscription health, and fee-payer balance —
/// in one JSON call instead of regex-scraping raw `/metrics` text (which the
/// panel used to do; fragile, and broke silently if the text format ever
/// reordered fields).
async fn metrics_summary(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::telemetry::worker_metrics::*;
    use std::sync::atomic::Ordering;

    let now = now_secs();
    let last_push = ER_LAST_PUSH_UNIX.load(Ordering::Relaxed);

    // Age of a heartbeat, or None when the worker has never ticked. `None`
    // rather than 0 matters: a worker that has not run once is a different
    // state from one that ticked this second, and 0 would render as the
    // healthiest possible value for the least healthy case.
    let age = |unix: u64| -> Option<u64> {
        if unix == 0 {
            None
        } else {
            Some(now.saturating_sub(unix))
        }
    };
    let n = |m: &std::sync::atomic::AtomicU64| m.load(Ordering::Relaxed);

    Ok(Json(json!({
        // ── headline ────────────────────────────────────────────────────
        "online_count": state.presence.count_in_game(),
        "games_in_progress": state.presence.count_games_in_progress(),
        "transactions_confirmed_solana": state.metrics.transactions_confirmed("solana"),
        "er_stale_delegated_count": SETTLEMENT_STALE_DELEGATED_GAUGE.load(Ordering::Relaxed),
        "er_subscription_connected": ER_SUBSCRIPTION_CONNECTED.load(Ordering::Relaxed) == 1,
        "er_last_push_age_seconds": age(last_push),
        "feepayer_balance_lamports": state.metrics.feepayer_balance(0),

        // ── worker detail ───────────────────────────────────────────────
        // Everything below was already being collected by the workers and
        // exported to Prometheus, but had no path into the admin panel — so
        // it was only visible to someone with a Grafana session. Grouped by
        // the worker that owns it so the UI can panel them directly.
        "settlement": {
            "ticks_total": n(&SETTLEMENT_TICKS_TOTAL),
            "tick_millis": n(&SETTLEMENT_TICK_MILLIS),
            "last_tick_age_seconds": age(n(&SETTLEMENT_LAST_TICK_UNIX)),
            "games_scanned_total": n(&SETTLEMENT_GAMES_SCANNED_TOTAL),
            "finalized_total": n(&SETTLEMENT_FINALIZED_TOTAL),
            "undelegated_total": n(&SETTLEMENT_UNDELEGATED_TOTAL),
            "rpc_calls_total": n(&SETTLEMENT_RPC_CALLS_TOTAL),
            "redelegate_retried_total": n(&SETTLEMENT_REDELEGATE_RETRIED_TOTAL),
            "redelegate_failed_total": n(&SETTLEMENT_REDELEGATE_FAILED_TOTAL),
            "force_undelegated_awaiting_recovery": n(&FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL),
            "stuck_delegation_auto_recovered": n(&STUCK_DELEGATION_AUTO_RECOVERED_TOTAL)
        },
        "schedulers": {
            "tournament_last_tick_age_seconds": age(n(&TOURNAMENT_SCHEDULER_LAST_TICK_UNIX)),
            "prize_distributor_last_tick_age_seconds": age(n(&PRIZE_DISTRIBUTOR_LAST_TICK_UNIX)),
            "time_check_scheduled_total": n(&TIME_CHECK_SCHEDULED_TOTAL),
            "time_check_schedule_failed_total": n(&TIME_CHECK_SCHEDULE_FAILED_TOTAL),
            "time_check_cancelled_total": n(&TIME_CHECK_CANCELLED_TOTAL),
            "time_check_cancel_failed_total": n(&TIME_CHECK_CANCEL_FAILED_TOTAL)
        },
        "anticheat": {
            "queue_depth": n(&ANTICHEAT_QUEUE_DEPTH),
            "enqueued_total": n(&ANTICHEAT_ENQUEUED_TOTAL),
            "dropped_total": n(&ANTICHEAT_DROPPED_TOTAL),
            "screened_out_total": n(&ANTICHEAT_SCREENED_OUT_TOTAL),
            "telemetry_discarded_total": n(&TELEMETRY_DISCARDED_TOTAL),
            "linkage_flagged_total": n(&LINKAGE_FLAGGED_TOTAL),
            "linkage_hard_blocked_total": n(&LINKAGE_HARD_BLOCKED_TOTAL)
        },
        "prizes": {
            "distributed_total": n(&PRIZE_DISTRIBUTED_TOTAL),
            "held_total": n(&PRIZE_DISTRIBUTION_HELD_TOTAL),
            "flagged_total": n(&PRIZE_DISTRIBUTION_FLAGGED_TOTAL),
            "awaiting_approval_total": n(&PRIZE_DISTRIBUTION_AWAITING_APPROVAL_TOTAL)
        },
        "guards": {
            "auth_unconfigured_relay_rejected_total": n(&AUTH_UNCONFIGURED_RELAY_REJECTED_TOTAL),
            "session_create_throttled_total": n(&SESSION_CREATE_THROTTLED_TOTAL),
            "sponsorship_budget_exhausted_total": n(&SPONSORSHIP_BUDGET_EXHAUSTED_TOTAL),
            "rates_fetch_failed_total": n(&RATES_FETCH_FAILED_TOTAL),
            "rates_sanity_rejected_total": n(&RATES_SANITY_REJECTED_TOTAL),
            "rates_source_divergence_total": n(&RATES_SOURCE_DIVERGENCE_TOTAL)
        }
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

/// Real certificate check: reads `TLS_DOMAIN` from the environment (the
/// deploy script's actual `-Domain`, previously hardcoded here as the wrong
/// `xfchess.gg` regardless of what was really deployed) and parses the live
/// PEM for a real `days_remaining` instead of a permanent `null`.
async fn tls_expiry() -> Result<Json<serde_json::Value>, StatusCode> {
    let domain = std::env::var("TLS_DOMAIN").unwrap_or_default();
    if domain.is_empty() {
        return Ok(Json(json!([{
            "domain": null,
            "status": "no_cert",
            "days_remaining": null,
            "note": "TLS_DOMAIN not set in .env — nothing to check",
        }])));
    }

    let cert_path = format!("/etc/letsencrypt/live/{domain}/cert.pem");
    let Ok(pem_bytes) = std::fs::read(&cert_path) else {
        return Ok(Json(json!([{
            "domain": domain,
            "cert_path": cert_path,
            "status": "no_cert",
            "days_remaining": null,
            "note": "TLS not yet configured on this node",
        }])));
    };

    match parse_cert_not_after_unix(&pem_bytes) {
        Some(not_after_unix) => {
            let days_remaining = (not_after_unix - now_secs() as i64).div_euclid(86_400);
            Ok(Json(json!([{
                "domain": domain,
                "cert_path": cert_path,
                "status": "found",
                "days_remaining": days_remaining,
            }])))
        }
        None => Ok(Json(json!([{
            "domain": domain,
            "cert_path": cert_path,
            "status": "found",
            "days_remaining": null,
            "note": "cert file present but could not be parsed",
        }]))),
    }
}

/// Parses a PEM-encoded certificate file and returns its `notAfter` field as
/// a Unix timestamp (seconds).
fn parse_cert_not_after_unix(pem_bytes: &[u8]) -> Option<i64> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(pem_bytes).ok()?;
    let cert = pem.parse_x509().ok()?;
    Some(cert.validity().not_after.timestamp())
}

/// Logs a rotation request only — does not actually rotate any key. Real
/// authority-key rotation is a manual runbook (`ops/SECRETS_ROTATION.md`),
/// deliberately not wired to a button here.
async fn rotate_token() -> Result<Json<serde_json::Value>, StatusCode> {
    use rand::Rng;
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    let new_token = format!("xf_admin_{}", hex::encode(bytes));
    add_audit(None, "rotate_token", "admin_token", "rotated").await;
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
    add_audit(None, "ip_ban", &req.ip, &req.reason).await;
    info!("[admin] IP banned: {} reason={}", req.ip, req.reason);
    Ok(Json(json!({ "ok": true, "ip": req.ip })))
}

async fn list_ip_bans() -> Result<Json<serde_json::Value>, StatusCode> {
    let bans = IP_BANS.lock().map(|b| b.clone()).unwrap_or_default();
    Ok(Json(json!({ "bans": bans })))
}

async fn assign_dispute(
    State(state): State<AppState>,
    Path(game_id): Path<u64>,
    Json(req): Json<AssignDisputeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let flagged_repo = crate::db::repository::FlaggedGameRepository::new(state.store.pool());
    flagged_repo
        .assign(game_id as i64, &req.reviewer)
        .await
        .map_err(|e| {
            error!(
                "[admin] Failed to persist dispute assignment for game {}: {}",
                game_id, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    add_audit(
        Some(state.store.pool()),
        "assign_dispute",
        &format!("game_{}", game_id),
        &req.reviewer,
    )
    .await;
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
            }
        })
        .await;

    // Generate bracket and start
    store.generate_bracket(id).await;
    match store.start_tournament(id).await {
        Ok(()) => {
            add_audit(
                Some(state.store.pool()),
                "fill_bots",
                &format!("tournament_{}", id),
                &format!("added {} bots", fill_count),
            )
            .await;
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
