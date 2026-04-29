use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use swiss_pairing::{MatchResult, SwissRound, StandingsEntry};

use super::service::{SwissService, SwissServiceError};

/// Request to record a match result
#[derive(Deserialize)]
pub struct RecordResultReq {
    pub round: u8,
    pub board: u16,
    /// Result: "1-0", "0-1", "0.5-0.5"
    pub result: String,
}

/// Response with current round
#[derive(Serialize)]
pub struct CurrentRoundRes {
    pub round: u8,
    pub total_rounds: u8,
    pub is_active: bool,
}

/// Create Swiss tournament routes
pub fn swiss_routes() -> Router<SwissService> {
    Router::new()
        .route("/{id}/round", post(start_round))
        .route("/{id}/current-round", get(get_current_round))
        .route("/{id}/pairings/{round}", get(get_pairings))
        .route("/{id}/result", post(record_result))
        .route("/{id}/standings", get(get_standings))
}

/// POST /tournament/{id}/round - Start next round
async fn start_round(
    Path(id): Path<u64>,
    State(service): State<SwissService>,
) -> Result<Json<SwissRound>, StatusCode> {
    match service.start_round(id).await {
        Ok(round) => {
            tracing::info!("Started round {} for tournament {}", round.round, id);
            Ok(Json(round))
        }
        Err(SwissServiceError::TournamentNotFound) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::NotSwissFormat) => Err(StatusCode::BAD_REQUEST),
        Err(SwissServiceError::TournamentComplete) => Err(StatusCode::CONFLICT),
        Err(e) => {
            tracing::error!("Error starting round: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /tournament/{id}/current-round - Get current round info
async fn get_current_round(
    Path(id): Path<u64>,
    State(service): State<SwissService>,
) -> Result<Json<CurrentRoundRes>, StatusCode> {
    match service.get_current_round(id).await {
        Ok(round) => Ok(Json(CurrentRoundRes {
            round,
            total_rounds: 0, // Would need to fetch from tournament
            is_active: round > 0,
        })),
        Err(SwissServiceError::TournamentNotFound) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::NotSwissFormat) => Err(StatusCode::BAD_REQUEST),
        Err(e) => {
            tracing::error!("Error getting current round: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /tournament/{id}/pairings/{round} - Get pairings for a round
async fn get_pairings(
    Path((id, round)): Path<(u64, u8)>,
    State(service): State<SwissService>,
) -> Result<Json<SwissRound>, StatusCode> {
    match service.get_pairings(id, round).await {
        Ok(Some(round_data)) => Ok(Json(round_data)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::TournamentNotFound) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::NotSwissFormat) => Err(StatusCode::BAD_REQUEST),
        Err(e) => {
            tracing::error!("Error getting pairings: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /tournament/{id}/result - Record a match result
async fn record_result(
    Path(id): Path<u64>,
    State(service): State<SwissService>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<Vec<StandingsEntry>>, StatusCode> {
    // Parse result string
    let result = match req.result.as_str() {
        "1-0" | "1-0 " => MatchResult::WhiteWin,
        "0-1" | "0-1 " => MatchResult::BlackWin,
        "0.5-0.5" | "1/2-1/2" | "draw" => MatchResult::Draw,
        "bye" | "BYE" => MatchResult::Bye,
        _ => {
            tracing::warn!("Invalid result format: {}", req.result);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match service.record_result(id, req.round, req.board, result).await {
        Ok(standings) => Ok(Json(standings)),
        Err(SwissServiceError::TournamentNotFound) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::NotSwissFormat) => Err(StatusCode::BAD_REQUEST),
        Err(SwissServiceError::InvalidRound(_)) => Err(StatusCode::BAD_REQUEST),
        Err(SwissServiceError::InvalidBoard(_)) => Err(StatusCode::BAD_REQUEST),
        Err(e) => {
            tracing::error!("Error recording result: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /tournament/{id}/standings - Get current standings
async fn get_standings(
    Path(id): Path<u64>,
    State(service): State<SwissService>,
) -> Result<Json<Vec<StandingsEntry>>, StatusCode> {
    match service.get_standings(id).await {
        Ok(standings) => Ok(Json(standings)),
        Err(SwissServiceError::TournamentNotFound) => Err(StatusCode::NOT_FOUND),
        Err(SwissServiceError::NotSwissFormat) => Err(StatusCode::BAD_REQUEST),
        Err(e) => {
            tracing::error!("Error getting standings: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
