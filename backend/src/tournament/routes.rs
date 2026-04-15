use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::signing::{AppState, TournamentStore};
use crate::signing::tournament_store::{MatchStatus, TournamentMatch, TournamentRecord};

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
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
    Json(req): Json<CreateTournamentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let record = TournamentRecord::new(req.tournament_id, req.name.clone(), req.entry_fee_lamports);
    store.create(record).await;
    info!("[tournament] Created tournament {} '{}'", req.tournament_id, req.name);
    Ok(Json(serde_json::json!({ "ok": true, "tournament_id": req.tournament_id })))
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
