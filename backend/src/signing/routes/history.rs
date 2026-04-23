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
    Router::new().route("/games/history/:wallet", get(get_game_history))
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
