//! HTTP handlers for Swiss-format tournaments.
//!
//! Exposes round management endpoints — start/current round, pairings,
//! result recording, standings, absence/withdrawal, forbidden pairings,
//! manual pairings, and result overrides — mounted under `/tournament/{id}/...`
//! by the signing service router. Handlers delegate to [`SwissService`]
//! for pairing generation, scoring, and state persistence, and translate
//! service errors into appropriate HTTP status codes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use swiss_pairing::{MatchResult, SwissRound, StandingsEntry};

use crate::signing::AppState;
use super::service::SwissServiceError;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RecordResultReq {
    pub round: u8,
    pub board: u16,
    /// Result: "1-0", "0-1", "0.5-0.5", "forfeit-white", "forfeit-black"
    pub result: String,
}

#[derive(Deserialize)]
pub struct AbsentReq {
    pub player_id: String,
    pub round: u8,
}

#[derive(Deserialize)]
pub struct WithdrawReq {
    pub player_id: String,
}

#[derive(Deserialize)]
pub struct RejoinReq {
    pub player_id: String,
}

#[derive(Deserialize)]
pub struct ForbiddenPairReq {
    pub player_a: String,
    pub player_b: String,
}

#[derive(Deserialize)]
pub struct ManualPairReq {
    pub white: String,
    pub black: String,
}

#[derive(Deserialize)]
pub struct OverrideResultReq {
    pub round: u8,
    pub board: u16,
    pub result: String,
}

#[derive(Serialize)]
pub struct CurrentRoundRes {
    pub round: u8,
    pub total_rounds: u8,
    pub is_active: bool,
}

// ── Router ───────────────────────────────────────────────────────────────────

/// Read-only Swiss routes — no authentication required
pub fn swiss_read_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/current-round", get(get_current_round))
        .route("/{id}/pairings/{round}", get(get_pairings))
        .route("/{id}/standings", get(get_standings))
}

/// State-mutating Swiss routes — must be wrapped with require_api_key by the caller
pub fn swiss_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/round", post(start_round))
        .route("/{id}/result", post(record_result))
        .route("/{id}/result", put(override_result))
        .route("/{id}/absent", post(mark_absent))
        .route("/{id}/withdraw", post(withdraw_player))
        .route("/{id}/rejoin", post(rejoin_player))
        .route("/{id}/forbidden-pair", post(add_forbidden_pair))
        .route("/{id}/forbidden-pair", delete(remove_forbidden_pair))
        .route("/{id}/manual-pair", post(add_manual_pairing))
        .route("/{id}/manual-pair", delete(remove_manual_pairing))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_result(s: &str) -> Option<MatchResult> {
    match s {
        "1-0" | "1-0 " => Some(MatchResult::WhiteWin),
        "0-1" | "0-1 " => Some(MatchResult::BlackWin),
        "0.5-0.5" | "1/2-1/2" | "draw" => Some(MatchResult::Draw),
        "bye" | "BYE" => Some(MatchResult::Bye),
        "forfeit-white" | "forfeit_white" => Some(MatchResult::ForfeitWhiteWin),
        "forfeit-black" | "forfeit_black" => Some(MatchResult::ForfeitBlackWin),
        _ => None,
    }
}

fn swiss_err(e: SwissServiceError) -> StatusCode {
    match e {
        SwissServiceError::TournamentNotFound => StatusCode::NOT_FOUND,
        SwissServiceError::NotSwissFormat => StatusCode::BAD_REQUEST,
        SwissServiceError::TournamentComplete => StatusCode::CONFLICT,
        SwissServiceError::InvalidRound(_) => StatusCode::BAD_REQUEST,
        SwissServiceError::InvalidBoard(_) => StatusCode::BAD_REQUEST,
        SwissServiceError::PlayerNotFound(_) => StatusCode::NOT_FOUND,
        SwissServiceError::PlayerWithdrawn => StatusCode::CONFLICT,
        e => {
            tracing::error!("Swiss service error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /tournament/{id}/round - Start next round
async fn start_round(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<SwissRound>, StatusCode> {
    state.swiss_service.start_round(id).await.map(Json).map_err(swiss_err)
}

/// GET /tournament/{id}/current-round
async fn get_current_round(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<CurrentRoundRes>, StatusCode> {
    let service = &state.swiss_service;
    let round = service.get_current_round(id).await.map_err(swiss_err)?;
    let total_rounds = service.get_total_rounds(id).await.unwrap_or(0);
    Ok(Json(CurrentRoundRes {
        round,
        total_rounds,
        is_active: round > 0,
    }))
}

/// GET /tournament/{id}/pairings/{round}
async fn get_pairings(
    Path((id, round)): Path<(u64, u8)>,
    State(state): State<AppState>,
) -> Result<Json<SwissRound>, StatusCode> {
    match state.swiss_service.get_pairings(id, round).await {
        Ok(Some(r)) => Ok(Json(r)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => Err(swiss_err(e)),
    }
}

/// POST /tournament/{id}/result - Record a match result
async fn record_result(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<Vec<StandingsEntry>>, StatusCode> {
    let result = parse_result(&req.result).ok_or_else(|| {
        tracing::warn!("Invalid result format: {}", req.result);
        StatusCode::BAD_REQUEST
    })?;
    state
        .swiss_service
        .record_result(id, req.round, req.board, result)
        .await
        .map(Json)
        .map_err(swiss_err)
}

/// GET /tournament/{id}/standings
async fn get_standings(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<Vec<StandingsEntry>>, StatusCode> {
    state.swiss_service.get_standings(id).await.map(Json).map_err(swiss_err)
}

/// POST /tournament/{id}/absent  — Gap 1
async fn mark_absent(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<AbsentReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .mark_absent(id, &req.player_id, req.round)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// POST /tournament/{id}/withdraw  — Gap 2
async fn withdraw_player(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<WithdrawReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .withdraw_player(id, &req.player_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// POST /tournament/{id}/rejoin  — Gap 3
async fn rejoin_player(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RejoinReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .rejoin_player(id, &req.player_id)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// POST /tournament/{id}/forbidden-pair  — Gap 6
async fn add_forbidden_pair(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ForbiddenPairReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .add_forbidden_pair(id, &req.player_a, &req.player_b)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// DELETE /tournament/{id}/forbidden-pair  — Gap 6
async fn remove_forbidden_pair(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ForbiddenPairReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .remove_forbidden_pair(id, &req.player_a, &req.player_b)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// POST /tournament/{id}/manual-pair  — Gap 6
async fn add_manual_pairing(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ManualPairReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .add_manual_pairing(id, &req.white, &req.black)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// DELETE /tournament/{id}/manual-pair  — Gap 6
async fn remove_manual_pairing(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<ManualPairReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .swiss_service
        .remove_manual_pairing(id, &req.white, &req.black)
        .await
        .map(|_| StatusCode::OK)
        .map_err(swiss_err)
}

/// PUT /tournament/{id}/result  — Gap 7 (admin-gated result override)
async fn override_result(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<OverrideResultReq>,
) -> Result<Json<Vec<StandingsEntry>>, StatusCode> {
    let result = parse_result(&req.result).ok_or_else(|| {
        tracing::warn!("Invalid result format: {}", req.result);
        StatusCode::BAD_REQUEST
    })?;
    state
        .swiss_service
        .override_result(id, req.round, req.board, result)
        .await
        .map(Json)
        .map_err(swiss_err)
}
