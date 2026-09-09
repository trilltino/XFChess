use axum::{extract::{Path, State}, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::signing::storage::{OfflineTournamentRecord, OfflineTournamentStore};
use crate::signing::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateOfflineTournamentRequest {
    pub tournament_id: String,
    pub name: String,
    pub format: String,
    pub state: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOfflineTournamentRequest {
    pub status: String,
    pub state: serde_json::Value,
    pub revision: i64,
}

#[derive(Debug, Serialize)]
pub struct OfflineEventSummary {
    pub tournament_id: String,
    pub name: String,
    pub format: String,
    pub status: String,
    pub revision: i64,
    pub updated_at: i64,
    pub participants: Vec<String>,
}

fn timestamp() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }
fn valid_format(format: &str) -> bool { matches!(format, "single_elimination" | "swiss") }
fn valid_status(status: &str) -> bool { matches!(status, "draft" | "published" | "active" | "completed" | "cancelled") }

pub fn public_routes() -> Router<AppState> {
    Router::new().route("", get(list_events)).route("/{id}", get(get_event))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new().route("", get(list_admin_events).post(create_event)).route("/{id}", get(get_admin_event).post(update_event))
}

async fn list_events(State(state): State<AppState>) -> Result<Json<Vec<OfflineEventSummary>>, StatusCode> {
    let records = OfflineTournamentStore::new(state.store.pool()).list().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(records.into_iter().filter(|r| r.status != "draft").map(summary).collect()))
}

async fn get_event(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<OfflineTournamentRecord>, StatusCode> {
    OfflineTournamentStore::new(state.store.pool()).get(&id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.filter(|r| r.status != "draft").map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn list_admin_events(State(state): State<AppState>) -> Result<Json<Vec<OfflineEventSummary>>, StatusCode> {
    let records = OfflineTournamentStore::new(state.store.pool()).list().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(records.into_iter().map(summary).collect()))
}

async fn get_admin_event(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<OfflineTournamentRecord>, StatusCode> {
    OfflineTournamentStore::new(state.store.pool()).get(&id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn create_event(State(state): State<AppState>, Json(req): Json<CreateOfflineTournamentRequest>) -> Result<Json<OfflineTournamentRecord>, (StatusCode, String)> {
    if req.tournament_id.trim().is_empty() || req.name.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "id and name are required".to_string())); }
    if !valid_format(&req.format) { return Err((StatusCode::BAD_REQUEST, "format must be single_elimination or swiss".to_string())); }
    let now = timestamp();
    let record = OfflineTournamentRecord { tournament_id: req.tournament_id, name: req.name, format: req.format, status: "draft".to_string(), state: req.state, revision: 0, created_at: now, updated_at: now };
    OfflineTournamentStore::new(state.store.pool()).create(&record).await.map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    Ok(Json(record))
}

async fn update_event(Path(id): Path<String>, State(state): State<AppState>, Json(req): Json<UpdateOfflineTournamentRequest>) -> Result<Json<OfflineTournamentRecord>, (StatusCode, String)> {
    if !valid_status(&req.status) { return Err((StatusCode::BAD_REQUEST, "invalid offline event status".to_string())); }
    let store = OfflineTournamentStore::new(state.store.pool());
    let current = store.get(&id).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "storage failure".to_string()))?.ok_or((StatusCode::NOT_FOUND, "offline event not found".to_string()))?;
    if req.revision != current.revision + 1 { return Err((StatusCode::CONFLICT, format!("revision conflict: expected {}", current.revision + 1))); }
    let updated_at = timestamp();
    if !store.update_state(&id, &req.status, &req.state, req.revision, updated_at).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "storage failure".to_string()))? { return Err((StatusCode::CONFLICT, "event changed; reload before updating".to_string())); }
    Ok(Json(OfflineTournamentRecord { status: req.status, state: req.state, revision: req.revision, updated_at, ..current }))
}

fn summary(record: OfflineTournamentRecord) -> OfflineEventSummary {
    let participants = record.state.get("participants").and_then(|v| v.as_array()).map(|items| items.iter().filter_map(|item| item.get("name").or_else(|| item.get("identity"))).filter_map(|v| v.as_str()).map(str::to_string).collect()).unwrap_or_default();
    OfflineEventSummary { tournament_id: record.tournament_id, name: record.name, format: record.format, status: record.status, revision: record.revision, updated_at: record.updated_at, participants }
}
