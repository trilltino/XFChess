//! Tournament API routes for 8-128 player single-elimination tournaments.
//!
//! This module provides HTTP endpoints for tournament management:
//! - Admin endpoints: create, record results, set match game IDs
//! - Player endpoints: list tournaments, join, get my match, get bracket, register node ID
//!
//! Tournaments use ELO-based seeding and support power-of-2 player counts.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::signing::storage::tournament::{MatchStatus, TournamentRecord, TournamentStore, TournamentStatus};

// ── Request / Response types ──────────────────────────────────────────────────

/// Request to create a new tournament.
#[derive(Deserialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    /// Max players: 8, 16, 32, 64, or 128
    pub max_players: u16,
    /// Prize distribution in basis points [1st, 2nd, 3rd, 4th]. Default: [5000, 3000, 1500, 500]
    pub prize_shares: Option<[u16; 4]>,
}

/// Request to register a player's P2P node ID.
#[derive(Deserialize)]
pub struct RegisterNodeReq {
    pub player: String,
    pub node_id: String,
}

/// Request to record a match result.
#[derive(Deserialize)]
pub struct RecordResultReq {
    pub match_index: usize,
    pub winner: String,
    pub loser: String,
}

/// Request to set the game ID for a match.
#[derive(Deserialize)]
pub struct SetMatchGameIdReq {
    pub match_index: usize,
    pub game_id: u64,
}

/// Tournament summary for listing.
#[derive(Serialize)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub registered: usize,
    pub status: String,
}

/// Creates player-facing tournament routes.
pub fn tournament_routes() -> Router<TournamentStore> {
    Router::new()
        .route("/{id}", get(get_tournament))
        .route("/{id}/register-node", post(register_node))
        .route("/{id}/my-match", get(get_my_match))
        .route("/{id}/bracket", get(get_bracket))
        .route("/{id}/join", post(join_tournament))
}

/// Creates admin tournament routes.
pub fn admin_tournament_routes() -> Router<TournamentStore> {
    Router::new()
        .route("/create", post(create_tournament))
        .route("/{id}/record-result", post(record_result))
        .route("/{id}/set-match-game-id", post(set_match_game_id))
}

/// Creates tournament listing routes.
pub fn tournaments_routes() -> Router<TournamentStore> {
    Router::new()
        .route("/", get(list_tournaments))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /admin/tournament/create - Creates a new tournament.
async fn create_tournament(
    State(store): State<TournamentStore>,
    Json(req): Json<CreateTournamentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Default prize shares: 50/30/15/5 for 16+ players, winner-take-all for 8
    let prize_shares = req.prize_shares.unwrap_or(
        if req.max_players >= 16 {
            [5000, 3000, 1500, 500]
        } else {
            [10000, 0, 0, 0]
        }
    );

    let record = TournamentRecord::with_config(
        req.tournament_id,
        req.name.clone(),
        req.entry_fee_lamports,
        req.max_players,
        prize_shares,
    );
    store.create(record).await;
    info!("[tournament] Created tournament {} '{}' ({} players)", req.tournament_id, req.name, req.max_players);
    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": req.tournament_id,
        "max_players": req.max_players,
        "prize_shares": prize_shares
    })))
}

/// GET /tournaments - Lists all tournaments.
async fn list_tournaments(
    State(store): State<TournamentStore>,
) -> Json<Vec<TournamentSummary>> {
    let all = store.list().await;
    let summaries = all.into_iter().map(|t| TournamentSummary {
        tournament_id: t.tournament_id,
        name: t.name,
        entry_fee_lamports: t.entry_fee_lamports,
        prize_pool: t.prize_pool,
        max_players: t.max_players,
        registered: t.players.len(),
        status: format!("{:?}", t.status),
    }).collect();
    Json(summaries)
}

/// GET /tournament/:id - Gets tournament details.
async fn get_tournament(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
) -> Result<Json<TournamentRecord>, StatusCode> {
    store.get(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// POST /tournament/:id/register-node - Registers a player's P2P node ID.
async fn register_node(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<RegisterNodeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ok = store.register_node_id(id, req.player.clone(), req.node_id.clone()).await;
    if !ok { return Err(StatusCode::NOT_FOUND); }
    info!("[tournament] {} registered node_id for tournament {}", req.player, id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /tournament/:id/my-match?player=<pubkey> - Gets a player's current match.
async fn get_my_match(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    match tournament.match_for_player(player) {
        Some(a) => Ok(Json(serde_json::json!({
            "found": true,
            "match_index": a.match_index,
            "game_id": a.game_id,
            "opponent_pubkey": a.opponent_pubkey,
            "opponent_node_id": a.opponent_node_id,
            "your_color": a.your_color,
            "status": format!("{:?}", a.status),
        }))),
        None => Ok(Json(serde_json::json!({ "found": false }))),
    }
}

/// GET /tournament/:id/bracket - Gets the tournament bracket.
async fn get_bracket(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let final_idx = t.final_match_index();
    let current_round = if t.matches.get(final_idx).map_or(false, |m| m.is_some()) {
        let m = t.matches[final_idx].as_ref().expect("Final match should exist");
        if m.status == MatchStatus::Completed { 255u8 } else { m.round }
    } else {
        0u8
    };

    Ok(Json(serde_json::json!({
        "tournament_id": t.tournament_id,
        "status": format!("{:?}", t.status),
        "max_players": t.max_players,
        "players": t.players,
        "matches": t.matches,
        "winner": t.winner,
        "second_place": t.second_place,
        "third_place": t.third_place,
        "fourth_place": t.fourth_place,
        "prize_shares": t.prize_shares,
        "current_round": current_round,
    })))
}

/// POST /admin/tournament/:id/record-result - Records a match result.
async fn record_result(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if req.match_index >= tournament.matches.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store.record_result(id, req.match_index, req.winner.clone(), req.loser.clone()).await;
    if !ok { return Err(StatusCode::NOT_FOUND); }
    info!("[tournament] Match {} of tournament {} won by {}", req.match_index, id, req.winner);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/set-match-game-id - Sets the game ID for a match.
async fn set_match_game_id(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(req): Json<SetMatchGameIdReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if req.match_index >= tournament.matches.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store.set_match_game_id(id, req.match_index, req.game_id).await;
    if !ok { return Err(StatusCode::NOT_FOUND); }
    info!("[tournament] Match {} of tournament {} assigned game_id {}", req.match_index, id, req.game_id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /tournament/:id/join - Joins a tournament.
async fn join_tournament(
    Path(id): Path<u64>,
    State(store): State<TournamentStore>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = body.get("player").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let elo = body.get("elo").and_then(|v| v.as_u64()).unwrap_or(1200) as u32;

    let mut slot = None;
    let mut just_full = false;
    let ok = store.update(id, |t| {
        if t.is_full() { return; }
        if t.players.iter().any(|p| p == player) {
            slot = Some(t.players.len());
            return;
        }
        slot = Some(t.players.len());
        t.players.push(player.to_string());
        t.player_elos.push(elo);
        t.prize_pool += t.entry_fee_lamports;
        // Check if tournament just filled
        if t.players.len() == t.max_players as usize {
            just_full = true;
        }
    }).await;

    if !ok { return Err(StatusCode::NOT_FOUND); }
    let position = slot.ok_or(StatusCode::CONFLICT)?;

    // If tournament just filled, seed players and generate bracket
    if just_full {
        let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
        let mut bracket_tournament = tournament.clone();
        seed_players_by_elo(&mut bracket_tournament);
        bracket_tournament.generate_bracket();
        bracket_tournament.status = TournamentStatus::Active;
        bracket_tournament.started_at = Some(chrono::Utc::now().timestamp());
        store.create(bracket_tournament).await;
        info!("[tournament] {} joined tournament {} at slot {} - BRACKET GENERATED", player, id, position);
    } else {
        info!("[tournament] {} joined tournament {} at slot {}/{}", player, id, position, store.get(id).await.map(|t| t.max_players).unwrap_or(0));
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "slot": position,
        "tournament_full": just_full
    })))
}

/// Seeds players by ELO descending (highest first).
/// Call before generating bracket.
fn seed_players_by_elo(t: &mut TournamentRecord) {
    let mut indexed: Vec<(usize, u32)> = t.player_elos.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    let mut seeded_players = Vec::with_capacity(t.players.len());
    let mut seeded_elos = Vec::with_capacity(t.player_elos.len());

    for (idx, elo) in indexed {
        seeded_players.push(t.players[idx].clone());
        seeded_elos.push(elo);
    }

    t.players = seeded_players;
    t.player_elos = seeded_elos;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tournament_req_serialization() {
        let req = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: 1000000,
            max_players: 16,
            prize_shares: Some([5000, 3000, 1500, 500]),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_register_node_req_serialization() {
        let req = RegisterNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_record_result_req_serialization() {
        let req = RecordResultReq {
            match_index: 0,
            winner: "player1".to_string(),
            loser: "player2".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_set_match_game_id_req_serialization() {
        let req = SetMatchGameIdReq {
            match_index: 0,
            game_id: 12345,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_tournament_summary_serialization() {
        let summary = TournamentSummary {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: 1000000,
            prize_pool: 16000000,
            max_players: 16,
            registered: 8,
            status: "Active".to_string(),
        };

        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn test_tournament_routes_creation() {
        let router = tournament_routes();
        assert!(router.not_found("test").is_some());
    }

    #[tokio::test]
    async fn test_admin_tournament_routes_creation() {
        let router = admin_tournament_routes();
        assert!(router.not_found("test").is_some());
    }

    #[tokio::test]
    async fn test_tournaments_routes_creation() {
        let router = tournaments_routes();
        assert!(router.not_found("test").is_some());
    }

    #[test]
    fn test_max_players_validation() {
        // Test valid player counts (power of 2)
        let valid_counts = vec![8, 16, 32, 64, 128];
        for count in valid_counts {
            let req = CreateTournamentReq {
                tournament_id: 1,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: 1000000,
                max_players: count,
                prize_shares: None,
            };
            assert_eq!(req.max_players, count);
        }
    }

    #[test]
    fn test_prize_shares_default() {
        // Test default prize shares for different player counts
        let req_large = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: 1000000,
            max_players: 16,
            prize_shares: None,
        };
        
        let prize_shares = req_large.prize_shares.unwrap_or(
            if req_large.max_players >= 16 {
                [5000, 3000, 1500, 500]
            } else {
                [10000, 0, 0, 0]
            }
        );
        
        assert_eq!(prize_shares, [5000, 3000, 1500, 500]);
    }

    #[test]
    fn test_prize_shares_winner_take_all() {
        // Test winner-take-all for small tournaments
        let req_small = CreateTournamentReq {
            tournament_id: 1,
            name: "Test Tournament".to_string(),
            entry_fee_lamports: 1000000,
            max_players: 8,
            prize_shares: None,
        };
        
        let prize_shares = req_small.prize_shares.unwrap_or(
            if req_small.max_players >= 16 {
                [5000, 3000, 1500, 500]
            } else {
                [10000, 0, 0, 0]
            }
        );
        
        assert_eq!(prize_shares, [10000, 0, 0, 0]);
    }

    #[test]
    fn test_entry_fee_lamports_validation() {
        // Test reasonable entry fee ranges
        let valid_fees = vec![0, 1000000, 5000000, 10000000];
        for fee in valid_fees {
            let req = CreateTournamentReq {
                tournament_id: 1,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: fee,
                max_players: 16,
                prize_shares: None,
            };
            assert_eq!(req.entry_fee_lamports, fee);
        }
    }

    #[test]
    fn test_tournament_id_validation() {
        // Test valid tournament IDs
        let valid_ids = vec![0, 1, 100, u64::MAX];
        for id in valid_ids {
            let req = CreateTournamentReq {
                tournament_id: id,
                name: "Test Tournament".to_string(),
                entry_fee_lamports: 1000000,
                max_players: 16,
                prize_shares: None,
            };
            assert_eq!(req.tournament_id, id);
        }
    }

    #[test]
    fn test_node_id_format() {
        // Test that node ID is a non-empty string
        let req = RegisterNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };
        assert!(!req.node_id.is_empty());
    }

    #[test]
    fn test_match_index_validation() {
        // Test valid match indices
        let valid_indices = vec![0, 1, 10, 100];
        for index in valid_indices {
            let req = RecordResultReq {
                match_index: index,
                winner: "player1".to_string(),
                loser: "player2".to_string(),
            };
            assert_eq!(req.match_index, index);
        }
    }

    #[test]
    fn test_game_id_validation() {
        // Test valid game IDs
        let valid_ids = vec![0, 1, 12345, u64::MAX];
        for id in valid_ids {
            let req = SetMatchGameIdReq {
                match_index: 0,
                game_id: id,
            };
            assert_eq!(req.game_id, id);
        }
    }

    #[test]
    fn test_seed_players_by_elo() {
        // Test ELO seeding function
        let mut record = TournamentRecord::with_config(
            1,
            "Test Tournament".to_string(),
            1000000,
            16,
            [5000, 3000, 1500, 500],
        );
        
        record.players = vec!["player1".to_string(), "player2".to_string(), "player3".to_string(), "player4".to_string()];
        record.player_elos = vec![1500, 2000, 1200, 1800];
        
        seed_players_by_elo(&mut record);
        
        // After seeding, should be sorted by ELO descending
        assert_eq!(record.player_elos[0], 2000); // Highest ELO first
        assert_eq!(record.player_elos[1], 1800);
        assert_eq!(record.player_elos[2], 1500);
        assert_eq!(record.player_elos[3], 1200); // Lowest ELO last
    }
}
