//! Read-only admin evidence. Unknown observations are never monetary/rating zeroes.
use super::{AppState, FeeReportQuery};
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use borsh::BorshDeserialize;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use sqlx::SqlitePool;
use std::{collections::HashMap, str::FromStr, sync::Mutex, time::{Duration, Instant}};

pub(super) fn query_error(error: impl std::fmt::Display) -> StatusCode {
    tracing::error!("[admin-reporting] query failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn window(q: &FeeReportQuery, now: i64) -> Result<(i64, i64), StatusCode> {
    let seconds = match q.period.as_deref().unwrap_or("week") {
        "day" => 86_400,
        "week" => 7 * 86_400,
        "month" => 30 * 86_400,
        "all" => now,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let to = q.to.unwrap_or(now);
    let from = q.from.unwrap_or_else(|| if q.period.as_deref() == Some("all") { 0 } else { to.saturating_sub(seconds).max(0) });
    if from < 0 || to <= from { return Err(StatusCode::BAD_REQUEST); }
    Ok((from, to))
}

pub(super) async fn fee_report(pool: &SqlitePool, q: FeeReportQuery) -> Result<Value, StatusCode> {
    let (from, to) = window(&q, chrono::Utc::now().timestamp())?;
    let (count, fees, stakes): (i64, i64, f64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(fee_lamports), 0), COALESCE(SUM(stake_amount), 0.0)
         FROM games WHERE status = 'completed' AND end_time >= ? AND end_time < ?")
        .bind(from).bind(to).fetch_one(pool).await.map_err(query_error)?;
    Ok(json!({"total_fee_lamports": fees, "total_fee_sol": fees as f64 / 1e9,
        "total_wagered_sol": stakes, "game_count": count,
        "period": q.period.as_deref().unwrap_or("week"), "from": from, "to": to,
        "date_basis": "end_time", "interval": "[from,to)", "observed_at": super::now_secs(),
        "source": "games", "status": "recorded", "chain_verified": false,
        "note": "Completed-game ledger totals; fee defaults and missing game records cannot establish chain revenue. Stake is not payout."}))
}

pub(super) async fn payouts(state: &AppState, q: FeeReportQuery) -> Result<Value, StatusCode> {
    use crate::signing::storage::money_action::MoneyActionRecord;
    let (from, to) = window(&q, chrono::Utc::now().timestamp())?;
    let rows = sqlx::query_as::<_, MoneyActionRecord>(
        "SELECT * FROM money_actions WHERE status = 'resolved'
         AND action_type IN ('finalize_game', 'claim_prize', 'distribute_prize', 'prize_distribution')
         AND signature IS NOT NULL AND TRIM(signature) != '' AND updated_at >= ? AND updated_at < ?
         ORDER BY updated_at DESC, id")
        .bind(from).bind(to).fetch_all(&state.store.pool()).await.map_err(query_error)?;
    let payouts: Vec<_> = rows.into_iter().map(|r| json!({
        "id": r.id, "game_id": r.game_id.map(|id| id.to_string()), "tournament_id": r.tournament_id,
        "wallet": r.wallet, "winner": null, "operation": r.action_type,
        "amount_lamports": null, "amount_sol": null, "tx_sig": r.signature,
        "settled_at": null, "reconciled_at": r.updated_at, "status": r.status,
        "source": "money_actions", "evidence": "reconciled_instruction",
        "amount_status": "not_recorded"
    })).collect();
    Ok(json!({"payouts": payouts, "from": from, "to": to, "date_basis": "updated_at",
        "observed_at": super::now_secs(), "coverage": "reconciled_money_actions_only",
        "note": "Confirmed payout operations; transferred amounts and recipients are not recorded. Multiple actions can reference one transaction; do not sum as distinct transfers."}))
}

#[derive(BorshDeserialize)]
struct ProfileRating {
    authority: [u8; 32],
    country: String,
    _wins: u32,
    _losses: u32,
    _draws: u32,
    _games_played: u32,
    elo: f64,
}

fn decode_rating(data: &[u8], wallet: &Pubkey) -> Option<f64> {
    let discriminator = Sha256::digest(b"account:PlayerProfile");
    if data.get(..8)? != &discriminator[..8] { return None; }
    let profile = ProfileRating::deserialize(&mut &data[8..]).ok()?;
    if profile.authority != wallet.to_bytes() || profile.country.len() > 2 || !profile.elo.is_finite() || profile.elo < 0.0 { return None; }
    Some(profile.elo / 100.0)
}

struct RatingEntry { at: Instant, value: Value }
static RATINGS: Lazy<Mutex<HashMap<String, RatingEntry>>> = Lazy::new(|| Mutex::new(HashMap::new()));
fn rating_key(state: &AppState, wallet: &str) -> String {
    format!("{}:{}:{wallet}", state.config.solana_rpc_url, state.program_id)
}
pub(super) fn cached_rating(state: &AppState, wallet: &str) -> Value {
    if let Ok(cache) = RATINGS.lock() {
        if let Some(entry) = cache.get(&rating_key(state, wallet)) {
            let ttl = if entry.value["status"] == "available" { 60 } else { 10 };
            if entry.at.elapsed() < Duration::from_secs(ttl) {
                let mut value = entry.value.clone();
                value["age_seconds"] = json!(entry.at.elapsed().as_secs());
                return value;
            }
        }
    }
    json!({"value": null, "status": "unknown", "source": "onchain_player_profile",
        "observed_at": null, "age_seconds": null, "reason": "not_cached_or_expired"})
}

async fn current_rating(state: &AppState, wallet: &str) -> Result<Value, StatusCode> {
    let pk = Pubkey::from_str(wallet).map_err(|_| StatusCode::BAD_REQUEST)?;
    let cached = cached_rating(state, wallet);
    if !cached["observed_at"].is_null() { return Ok(cached); }
    let program = state.program_id;
    let pda = Pubkey::find_program_address(&[b"profile", pk.as_ref()], &program).0;
    let rpc = state.solana_rpc.clone();
    let result = tokio::task::spawn_blocking(move || {
        rpc.get_account_with_commitment(&pda, solana_commitment_config::CommitmentConfig::confirmed())
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (value, status, reason, slot) = match result {
        Ok(response) => match response.value {
            None => (None, "unavailable", Some("profile_not_found"), Some(response.context.slot)),
            Some(account) if account.owner == program => match decode_rating(&account.data, &pk) {
                Some(rating) => (Some(rating), "available", None, Some(response.context.slot)),
                None => (None, "unknown", Some("invalid_profile_data"), Some(response.context.slot)),
            },
            Some(_) => (None, "unknown", Some("invalid_profile_owner"), Some(response.context.slot)),
        },
        Err(error) => {
            tracing::warn!("[admin-reporting] rating RPC failed: {error}");
            (None, "unknown", Some("rpc_error"), None)
        }
    };
    let result = json!({"value": value, "status": status, "reason": reason,
        "source": "onchain_player_profile", "profile_pda": pda.to_string(),
        "observed_at": super::now_secs(), "age_seconds": 0, "slot": slot, "commitment": "confirmed"});
    if let Ok(mut cache) = RATINGS.lock() {
        cache.retain(|_, entry| entry.at.elapsed() < Duration::from_secs(60));
        if cache.len() >= 4096 { cache.clear(); }
        cache.insert(rating_key(state, wallet), RatingEntry { at: Instant::now(), value: result.clone() });
    }
    Ok(result)
}

pub(super) async fn player_detail(State(state): State<AppState>, Path(wallet): Path<String>) -> Result<Json<Value>, StatusCode> {
    Pubkey::from_str(&wallet).map_err(|_| StatusCode::BAD_REQUEST)?;
    let pool = state.store.pool();
    let user: Option<(String, String)> = sqlx::query_as("SELECT username, kyc_status FROM users_v2 WHERE wallet = ? AND deleted_at IS NULL")
        .bind(&wallet).fetch_optional(&pool).await.map_err(query_error)?;
    let (games, wins, losses, draws): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(status = 'completed' AND winner = ?), 0),
         COALESCE(SUM(status = 'completed' AND winner IS NOT NULL AND winner != ?), 0),
         COALESCE(SUM(status = 'completed' AND winner IS NULL), 0)
         FROM games WHERE player_white = ? OR player_black = ?")
        .bind(&wallet).bind(&wallet).bind(&wallet).bind(&wallet).fetch_one(&pool).await.map_err(query_error)?;
    let rating = current_rating(&state, &wallet).await?;
    Ok(Json(json!({"wallet": wallet, "username": user.as_ref().map(|v| &v.0),
        "kyc_status": user.as_ref().map(|v| &v.1), "elo": rating["value"], "rating": rating,
        "games": {"total": games, "wins": wins, "losses": losses, "draws": draws},
        "provenance": {"identity": "users_v2", "games": "games", "coverage": "persisted_records_only", "observed_at": super::now_secs()}})))
}

pub(super) async fn pvp_summary(State(state): State<AppState>, Query(q): Query<FeeReportQuery>) -> Result<Json<Value>, StatusCode> {
    let (from, to) = window(&q, chrono::Utc::now().timestamp())?;
    let (total, completed, playing, stakes): (i64, i64, i64, f64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(status = 'completed'), 0), COALESCE(SUM(status = 'playing'), 0),
         COALESCE(SUM(stake_amount), 0.0) FROM games WHERE start_time >= ? AND start_time < ?")
        .bind(from).bind(to).fetch_one(&state.store.pool()).await.map_err(query_error)?;
    Ok(Json(json!({"game_count": total, "completed_count": completed, "playing_count": playing,
        "recorded_stake_sol": stakes, "from": from, "to": to, "date_basis": "start_time",
        "source": "games", "observed_at": super::now_secs(),
        "coverage": "persisted_games_including_tournament_games",
        "note": "The games schema has no authoritative PvP/tournament classification; these are all recorded games."})))
}

pub(super) async fn capabilities(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let mut sources = serde_json::Map::new();
    for (name, table) in [("game_db", "games"), ("moves_db", "moves"), ("braid_event_log", "game_event_log"),
        ("tournament_transactions", "tournament_transactions"), ("money_actions", "money_actions"),
        ("anti_cheat", "anticheat_verdicts"), ("moderation", "flagged_games"), ("sessions", "sessions"), ("disputes", "disputes")] {
        // Table names are fixed above, never supplied by the request.
        let result = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT 1 FROM {table} LIMIT 1"))).fetch_optional(&state.store.pool()).await;
        let status = match result { Ok(_) => "available", Err(ref e) => { tracing::warn!("[admin-reporting] capability {name}: {e}"); "unknown" } };
        sources.insert(name.into(), json!({"status": status, "checked_at": super::now_secs(),
            "source": table, "freshness": "query_at_request", "data_as_of": null}));
    }
    for name in ["pgn", "archive", "solana_tx_analyzer"] {
        sources.insert(name.into(), json!({"status": "unknown", "checked_at": null, "reason": "not_probed"}));
    }
    sources.insert("tournament_store".into(), json!({"status": "available", "source": "process_memory", "checked_at": super::now_secs(), "data_as_of": null}));
    Ok(Json(json!({"sources": sources, "observed_at": super::now_secs(),
        "environment": {"production": state.config.is_production(), "program_id": state.program_id.to_string(),
            "rpc": crate::signing::solana::redact_url(&state.config.solana_rpc_url)},
        "transaction_version_support": {
            "max_supported_transaction_version": crate::signing::solana::MAX_SUPPORTED_TX_VERSION,
            "v1_reads_enabled": true, "v1_read_required": crate::signing::solana::v1_read_required(),
            "v1_send_enabled": crate::signing::solana::TransactionBuildPolicy::v1_send_enabled(),
            "rpc_support_status": "unknown", "wallet_support_status": "unknown"},
        "rating_override": {"status": "unsupported"},
        "rating_reads": {"source": "onchain_player_profile", "list_policy": "cache_only", "detail_cache_ttl_seconds": 60}
    })))
}

pub(super) async fn game_sessions(pool: &SqlitePool, game_id: &str) -> Result<Value, sqlx::Error> {
    // Explicit allowlist: sessions also contains encrypted signing keypairs.
    let sessions: Vec<(i64, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT game_id, wallet, active, is_global, created_at FROM sessions WHERE CAST(game_id AS TEXT) = ?")
        .bind(game_id).fetch_all(pool).await?;
    let recovery: Vec<(String, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT session_id, status, last_activity, grace_period_ends FROM active_sessions WHERE CAST(game_id AS TEXT) = ?")
        .bind(game_id).fetch_all(pool).await?;
    Ok(json!({"available": true, "source": "sessions,active_sessions", "observed_at": super::now_secs(),
        "signing": sessions.into_iter().map(|(id, wallet, active, global, created)| json!({
            "game_id": id.to_string(), "wallet": wallet, "active": active != 0, "is_global": global != 0, "created_at": created
        })).collect::<Vec<_>>(),
        "recovery": recovery.into_iter().map(|(id, status, activity, grace)| json!({
            "session_id": id, "status": status, "last_activity": activity, "grace_period_ends": grace
        })).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_windows_validate_and_use_exclusive_end() {
        let mut q = FeeReportQuery { period: Some("week".into()), from: None, to: Some(1_000_000) };
        assert_eq!(window(&q, 2_000_000).unwrap(), (395_200, 1_000_000));
        q.from = Some(1_000_000);
        assert_eq!(window(&q, 2_000_000), Err(StatusCode::BAD_REQUEST));
        q.period = Some("typo".into());
        assert!(window(&q, 2_000_000).is_err());
    }

    #[test]
    fn rating_requires_valid_discriminator_authority_and_finite_value() {
        let wallet = Pubkey::new_unique();
        let mut data = Sha256::digest(b"account:PlayerProfile")[..8].to_vec();
        data.extend(wallet.to_bytes());
        data.extend(0u32.to_le_bytes()); // Empty country must not shift rating decoding.
        data.extend([0; 16]);
        data.extend(153_250f64.to_le_bytes());
        assert_eq!(decode_rating(&data, &wallet), Some(1532.5));
        assert_eq!(decode_rating(&data, &Pubkey::new_unique()), None);
        assert_eq!(decode_rating(&data[..20], &wallet), None);
        let end = data.len();
        data[end-8..].copy_from_slice(&f64::NAN.to_le_bytes());
        assert_eq!(decode_rating(&data, &wallet), None);
    }

    #[tokio::test]
    async fn fee_totals_include_all_matching_rows_and_propagate_errors() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let q = || FeeReportQuery { period: Some("all".into()), from: Some(10), to: Some(20) };
        assert!(fee_report(&pool, q()).await.is_err());
        sqlx::query("CREATE TABLE games (fee_lamports INTEGER, stake_amount REAL, status TEXT, end_time INTEGER)")
            .execute(&pool).await.unwrap();
        sqlx::query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<250)
            INSERT INTO games SELECT 7, 0.5, 'completed', 10 FROM n").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO games VALUES (999, 99, 'completed', 20), (999, 99, 'playing', 15), (999, 99, 'completed', 9)")
            .execute(&pool).await.unwrap();
        let report = fee_report(&pool, q()).await.unwrap();
        assert_eq!(report["game_count"], 250);
        assert_eq!(report["total_fee_lamports"], 1750);
        assert_eq!(report["total_wagered_sol"], 125.0);
    }
}
