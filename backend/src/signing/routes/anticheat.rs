//! Anti-cheat routes.
//!
//! GET  /anticheat/verdict/:game_id  — query the verdict for a finished game
//! GET  /anticheat/stats/:pubkey     — query rolling stats for a player

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use tracing::error;

use crate::signing::AppState;

pub fn anticheat_routes() -> Router<AppState> {
    Router::new()
        .route("/anticheat/verdict/{game_id}", get(get_verdict))
        .route("/anticheat/stats/{pubkey}", get(get_player_stats))
}

/// GET /anticheat/verdict/:game_id
pub async fn get_verdict(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();

    let row: Option<(String, String, String, String, f64, f64, String, String, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT white_pubkey, black_pubkey,
                    white_verdict, black_verdict,
                    white_score, black_score,
                    white_signals, black_signals,
                    report_path, analysed_at
             FROM anticheat_verdicts WHERE game_id = ?
             ORDER BY analysed_at DESC LIMIT 1"
        )
        .bind(&game_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            error!("[anticheat] verdict query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match row {
        None => Err(StatusCode::NOT_FOUND),
        Some((white_pk, black_pk, white_v, black_v, white_s, black_s, white_sig, black_sig, path, analysed_at)) =>
            Ok(Json(serde_json::json!({
                "game_id": game_id,
                "white": {
                    "pubkey": white_pk,
                    "verdict": white_v,
                    "score": white_s,
                    "signals": serde_json::from_str::<serde_json::Value>(&white_sig).unwrap_or_default(),
                },
                "black": {
                    "pubkey": black_pk,
                    "verdict": black_v,
                    "score": black_s,
                    "signals": serde_json::from_str::<serde_json::Value>(&black_sig).unwrap_or_default(),
                },
                "report_path": path,
                "analysed_at": analysed_at,
            }))),
    }
}

/// GET /anticheat/stats/:pubkey
pub async fn get_player_stats(
    State(state): State<AppState>,
    Path(pubkey): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();

    let row: Option<(i64, f64, String, String, i64, i64, i64)> =
        sqlx::query_as(
            "SELECT games_analysed, lifetime_cpl, last_30_cpls, last_30_t1s,
                    flags_received, reviews_received, last_updated
             FROM player_anticheat_stats WHERE pubkey = ?"
        )
        .bind(&pubkey)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            error!("[anticheat] stats query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match row {
        None => Err(StatusCode::NOT_FOUND),
        Some((games, cpl, cpls_json, t1s_json, flags, reviews, updated)) =>
            Ok(Json(serde_json::json!({
                "pubkey": pubkey,
                "games_analysed": games,
                "lifetime_cpl": cpl,
                "last_30_cpls": serde_json::from_str::<serde_json::Value>(&cpls_json).unwrap_or_default(),
                "last_30_t1s": serde_json::from_str::<serde_json::Value>(&t1s_json).unwrap_or_default(),
                "flags_received": flags,
                "reviews_received": reviews,
                "last_updated": updated,
            }))),
    }
}
