//! Game history route.
//!
//! GET /games/history/:wallet — returns last 20 games for a player wallet.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use tracing::error;

use crate::db::repository::GameRepository;
use crate::signing::AppState;

pub fn history_routes() -> Router<AppState> {
    Router::new()
        .route("/games/history/{wallet}", get(get_game_history))
        .route(
            "/games/history/username/{username}",
            get(get_game_history_by_username),
        )
        .route("/games/moves/{game_id}", get(get_game_moves))
        .route("/games/{game_id}/broadcast-delay", get(get_broadcast_delay))
        .route("/games/{game_id}/pgn", get(get_game_pgn))
        .route("/ratings/history/{wallet}", get(get_ratings_history))
}

/// GET /games/{game_id}/broadcast-delay — the game's public spectator delay in
/// seconds (0 = live). Spectator clients query this *before* deciding whether
/// to subscribe to the live P2P gossip feed: a non-zero delay means the only
/// permitted public source is the delay-gated HTTP move feed.
pub async fn get_broadcast_delay(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
) -> Json<serde_json::Value> {
    let repo = GameRepository::new(state.store.pool());
    let delay = repo.get_broadcast_delay(&game_id).await;
    Json(serde_json::json!({ "delay_secs": delay }))
}

pub async fn get_game_history(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    let games = repo.get_games_by_player(&wallet, 20).await.map_err(|e| {
        error!("[history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "games": games })))
}

pub async fn get_game_history_by_username(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    let games = repo
        .get_games_by_username(&username, 20)
        .await
        .map_err(|e| {
            error!("[history] DB query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({ "games": games })))
}

/// GET /games/moves/{game_id} — public (delayed) spectator feed.
///
/// Returns only moves at least the game's `broadcast_delay_secs` old, so a
/// live stream can't be used to ghost. For delay = 0 games (all casual/ranked
/// today) this is the full move list, unchanged. There is deliberately no
/// `live` bypass on this unauthenticated endpoint — that's exactly the feed an
/// accomplice would watch. Participants/casters get live via authorized paths.
pub async fn get_game_moves(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    let now_ts = chrono::Utc::now().timestamp();
    let moves = repo
        .get_moves_visible(&game_id, now_ts)
        .await
        .map_err(|e| {
            error!("[history] DB query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({ "moves": moves })))
}

/// GET /ratings/history/{wallet} — Returns the last 50 game results for ELO chart.
/// Each entry: {game_id, result: "win"|"loss"|"draw"|"unknown", opponent, timestamp, stake_amount}
pub async fn get_ratings_history(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    let games = repo.get_games_by_player(&wallet, 50).await.map_err(|e| {
        error!("[ratings/history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let history: Vec<serde_json::Value> = games
        .iter()
        .map(|g| {
            let result = match g.winner.as_deref() {
                Some(w) if w == wallet => "win",
                Some(_) => "loss",
                None if g.status == "draw" => "draw",
                None => "unknown",
            };
            let opponent = if g.player_white.as_deref() == Some(wallet.as_str()) {
                g.player_black.clone()
            } else {
                g.player_white.clone()
            };
            serde_json::json!({
                "game_id": g.id,
                "result": result,
                "opponent": opponent,
                "timestamp": g.start_time,
                "stake_amount": g.stake_amount,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "history": history })))
}

/// GET /games/{game_id}/pgn — Returns the PGN text for a game.
pub async fn get_game_pgn(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    // Try pre-assembled PGN first
    let pgn = repo.get_pgn_text(&game_id).await.map_err(|e| {
        error!("[history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(pgn_text) = pgn {
        return Ok(Json(serde_json::json!({ "pgn": pgn_text })));
    }

    // Fallback: assemble from stored SAN moves for live/in-progress games
    let moves = repo.get_moves(&game_id).await.map_err(|e| {
        error!("[history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    use nimzovich_engine::{PgnAssembler, PgnResult};
    let mut assembler = PgnAssembler::new();
    assembler
        .tag("Event", "XFChess Game")
        .tag("Site", "XFChess");
    for mv in moves {
        if let Some(san) = mv.move_san {
            assembler.add_move(san);
        }
    }
    assembler.set_result(PgnResult::Unfinished);
    let pgn_text = assembler.to_string();

    Ok(Json(serde_json::json!({ "pgn": pgn_text })))
}
