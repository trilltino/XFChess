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
        .route("/games/history/username/{username}", get(get_game_history_by_username))
        .route("/games/moves/{game_id}", get(get_game_moves))
        .route("/games/{game_id}/pgn", get(get_game_pgn))
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

    let games = repo.get_games_by_username(&username, 20).await.map_err(|e| {
        error!("[history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "games": games })))
}

pub async fn get_game_moves(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let repo = GameRepository::new(pool);

    let moves = repo.get_moves(&game_id).await.map_err(|e| {
        error!("[history] DB query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "moves": moves })))
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
    assembler.tag("Event", "XFChess Game").tag("Site", "XFChess");
    for mv in moves {
        if let Some(san) = mv.move_san {
            assembler.add_move(san);
        }
    }
    assembler.set_result(PgnResult::Unfinished);
    let pgn_text = assembler.to_string();

    Ok(Json(serde_json::json!({ "pgn": pgn_text })))
}
