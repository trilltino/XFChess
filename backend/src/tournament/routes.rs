use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use bcrypt::{hash, DEFAULT_COST, verify};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::info;

use crate::signing::{AppState, TournamentStore};
use crate::signing::tournament_store::{MatchStatus, TournamentMatch, TournamentRecord};

lazy_static::lazy_static! {
    static ref FAILED_ATTEMPTS: Arc<RwLock<HashMap<String, (u32, SystemTime)>>> = Arc::new(RwLock::new(HashMap::new()));
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterNodeReq {
    pub player: String,
    pub node_id: String,
}

#[derive(Deserialize)]
pub struct RecordResultReq {
    pub match_index: usize,
    pub winner: String,
}

#[derive(Deserialize)]
pub struct SetMatchGameIdReq {
    pub match_index: usize,
    pub game_id: u64,
}

#[derive(Serialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub registered: usize,
    pub status: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /admin/tournament/create
pub async fn create_tournament(
    State(store): State<TournamentStore>,
    Json(body): Json<CreateTournamentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let password_hash = if let Some(pw) = body.password {
        Some(hash(pw, DEFAULT_COST).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
    } else {
        None
    };

    let record = TournamentRecord::new(body.tournament_id, body.name.clone(), body.entry_fee_lamports);
    record.password_hash = password_hash;
    store.create(record).await;
    info!("[tournament] Created tournament {} '{}'", body.tournament_id, body.name);
    Ok(Json(serde_json::json!({ "ok": true, "tournament_id": body.tournament_id })))
}

/// GET /tournaments
pub async fn list_tournaments(
    State(store): State<TournamentStore>,
) -> Json<Vec<TournamentSummary>> {
    let all = store.list().await;
    let summaries: Vec<TournamentSummary> = all
        .into_iter()
        .map(|t| TournamentSummary {
            tournament_id: t.tournament_id,
            name: t.name,
            entry_fee_lamports: t.entry_fee_lamports,
            prize_pool: t.prize_pool,
            registered: t.players.len(),
            status: format!("{:?}", t.status),
        })
        .collect();
    Json(summaries)
}

/// GET /tournament/:id
pub async fn get_tournament(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
) -> Result<Json<TournamentRecord>, StatusCode> {
    store.get(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// POST /tournament/:id/register-node  { player, node_id }
pub async fn register_node(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<RegisterNodeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ok = store.register_node_id(id, req.player.clone(), req.node_id.clone()).await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    info!("[tournament] {} registered node_id for tournament {}", req.player, id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /tournament/:id/my-match?player=<pubkey>
pub async fn get_my_match(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    match tournament.match_for_player(player) {
        Some(assignment) => Ok(Json(serde_json::json!({
            "found": true,
            "match_index": assignment.match_index,
            "game_id": assignment.game_id,
            "opponent_pubkey": assignment.opponent_pubkey,
            "opponent_node_id": assignment.opponent_node_id,
            "your_color": assignment.your_color,
            "status": format!("{:?}", assignment.status),
        }))),
        None => Ok(Json(serde_json::json!({ "found": false }))),
    }
}

/// GET /tournament/:id/bracket
pub async fn get_bracket(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({
        "tournament_id": t.tournament_id,
        "status": format!("{:?}", t.status),
        "players": t.players,
        "matches": t.matches,
        "winner": t.winner,
        "current_round": if t.matches[2].is_some() { 1u8 } else { 0u8 },
    })))
}

/// POST /admin/tournament/:id/record-result  { match_index, winner }
pub async fn record_result(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.match_index > 2 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store.record_result(id, req.match_index, req.winner.clone()).await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    info!("[tournament] Match {} of tournament {} won by {}", req.match_index, id, req.winner);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/set-match-game-id  { match_index, game_id }
pub async fn set_match_game_id(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<SetMatchGameIdReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.match_index > 2 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store.set_match_game_id(id, req.match_index, req.game_id).await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    info!("[tournament] Match {} of tournament {} assigned game_id {}", req.match_index, id, req.game_id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/set-password  { password }
pub async fn set_tournament_password(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let password = body.get("password").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let hashed = hash(password, DEFAULT_COST).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ok = store.update(id, |t| {
        t.password_hash = Some(hashed.clone());
    }).await;

    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    info!("[tournament] Password set for tournament {}", id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /tournament/:id/join  { player, elo }  (backend-side registration — on-chain done by client)
pub async fn join_tournament(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = body.get("player").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let elo = body.get("elo")
        .and_then(|v| v.as_u64())
        .unwrap_or(1200) as u32; // Default ELO of 1200 if not provided

    // Password handling
    let supplied_pw = body.get("password").and_then(|v| v.as_str());

    // IP-based cooldown for failed password attempts
    let client_ip = "unknown".to_string(); // Placeholder: extract real IP from request headers if available
    let mut failed_attempts = FAILED_ATTEMPTS.write().await;
    let now = SystemTime::now();
    if let Some((count, last_time)) = failed_attempts.get(&client_ip) {
        if *count >= 3 {
            if let Ok(duration) = now.duration_since(*last_time) {
                if duration.as_secs() < 5 {
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                } else {
                    // Reset after cooldown period
                    failed_attempts.insert(client_ip.clone(), (0, now));
                }
            }
        }
    }

    // Check if tournament requires a password
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if let Some(hash) = &tournament.password_hash {
        match supplied_pw {
            Some(pw) => {
                match verify(pw, hash) {
                    Ok(true) => {
                        // Password correct, reset failed attempts
                        failed_attempts.insert(client_ip.clone(), (0, now));
                    },
                    Ok(false) | Err(_) => {
                        // Password incorrect or verification error
                        let entry = failed_attempts.entry(client_ip.clone()).or_insert((0, now));
                        entry.0 += 1;
                        entry.1 = now;
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                }
            },
            None => return Err(StatusCode::UNAUTHORIZED),
        }
    }

    let mut slot = None;
    let ok = store.update(id, |t| {
        if t.is_full() {
            return;
        }
        if t.players.iter().any(|p| p == player) {
            slot = Some(t.players.len());
            return;
        }
        slot = Some(t.players.len());
        t.players.push(player.to_string());
        t.player_elos.push(elo);
        t.prize_pool += t.entry_fee_lamports;

        // Auto-start: when 4 players join, seed bracket
        if t.players.len() == 4 {
            start_bracket(t);
        }
    }).await;

    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    let position = slot.ok_or(StatusCode::CONFLICT)?;
    info!("[tournament] {} joined tournament {} at slot {}", player, id, position);
    Ok(Json(serde_json::json!({ "ok": true, "slot": position })))
}

/// Seed the bracket when 4 players join: ELO-sorted, highest vs lowest in SF1.
fn start_bracket(t: &mut TournamentRecord) {
    let mut indexed: Vec<(usize, u32)> = t.player_elos.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1)); // descending ELO

    let sf1_white = t.players[indexed[0].0].clone();
    let sf1_black = t.players[indexed[3].0].clone();
    let sf2_white = t.players[indexed[1].0].clone();
    let sf2_black = t.players[indexed[2].0].clone();

    t.matches[0] = Some(TournamentMatch {
        match_index: 0,
        round: 0,
        player_white: Some(sf1_white),
        player_black: Some(sf1_black),
        winner: None,
        game_id: None,
        status: MatchStatus::Pending,
    });
    t.matches[1] = Some(TournamentMatch {
        match_index: 1,
        round: 0,
        player_white: Some(sf2_white),
        player_black: Some(sf2_black),
        winner: None,
        game_id: None,
        status: MatchStatus::Pending,
    });
    // Final is seeded empty until both SFs complete
    t.matches[2] = None;
    t.status = super::store::TournamentStatus::Active;
    t.started_at = Some(chrono::Utc::now().timestamp());
}

pub fn admin_routes() -> Router<TournamentStore> {
    Router::new()
        .route("/create", post(create_tournament))
        .route("/:id/set-password", post(set_tournament_password))
        .route("/:id/record-result", post(record_result))
        .route("/:id/set-match-game-id", post(set_match_game_id))
}
