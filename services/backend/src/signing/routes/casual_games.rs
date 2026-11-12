use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::signing::routes::auth::authed_wallet;
use crate::signing::AppState;

#[derive(Deserialize)]
pub struct CasualGameRequest {
    pub opponent_type: String,
    pub result: String,
    pub pgn: Option<String>,
}

#[derive(Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

pub fn casual_games_routes() -> Router<AppState> {
    Router::new().route("/api/games/casual", post(record_casual_game))
}

async fn record_casual_game(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CasualGameRequest>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let account_id = authed_wallet(&state, &headers).await?;

    if req.opponent_type != "bot" && req.opponent_type != "local_p2p" {
        return Err((
            StatusCode::BAD_REQUEST,
            "opponent_type must be 'bot' or 'local_p2p'".to_string(),
        ));
    }
    if !["win", "loss", "draw"].contains(&req.result.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "result must be 'win', 'loss', or 'draw'".to_string(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    state
        .store
        .record_casual_game(
            &account_id,
            &req.opponent_type,
            &req.result,
            req.pgn.as_deref(),
            now,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(OkResponse { ok: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_builds() {
        let _r: Router<AppState> = casual_games_routes();
    }
}
