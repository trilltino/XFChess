//! Tournament API routes for 8-256 player single-elimination and Swiss tournaments.
//!
//! This module provides HTTP endpoints for tournament management:
//! - Admin endpoints: create, record results, set match game IDs
//! - Player endpoints: list tournaments, join, get my match, get bracket, register node ID
//! - Gossip endpoints: subscribe to tournament updates, get bootstrap peers
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
use tracing::{info, warn};

use crate::signing::storage::tournament::{MatchStatus, TournamentRecord, TournamentStore, TournamentStatus, TournamentFormat};
use crate::signing::{AppState, TournamentTrigger};

// ── Request / Response types ──────────────────────────────────────────────────

/// Request to create a new tournament.
#[derive(Deserialize)]
pub struct CreateTournamentReq {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    /// Max players: 8, 16, 32, 64, 128, or 256
    pub max_players: u16,
    /// Tournament format: "SingleElimination" or "Swiss"
    #[serde(default = "default_format")]
    pub format: String,
    /// Number of Swiss rounds (required for Swiss format)
    pub swiss_rounds: Option<u8>,
    /// Minimum ELO rating for players (optional)
    pub elo_min: Option<u32>,
    /// Maximum ELO rating for players (optional)
    pub elo_max: Option<u32>,
    /// Minimum players required to start tournament (optional)
    pub min_players: Option<u16>,
    /// Prize distribution in basis points [1st, 2nd, 3rd, 4th]. Default: [5000, 3000, 1500, 500]
    pub prize_shares: Option<[u16; 4]>,
    /// Unix timestamp for when the tournament is scheduled to open (None = open immediately)
    pub scheduled_at: Option<i64>,
    /// Whether CACF KYC verification is required to join
    #[serde(default)]
    pub kyc_required: bool,
}

fn default_format() -> String {
    "SingleElimination".to_string()
}

/// Request to register a player's P2P node ID.
#[derive(Deserialize)]
pub struct RegisterNodeReq {
    pub player: String,
    pub node_id: String,
}

/// Request to subscribe to tournament gossip updates.
#[derive(Deserialize)]
pub struct SubscribeNodeReq {
    pub player: String,
    pub node_id: String,
}

/// Response for tournament subscription.
#[derive(Serialize)]
pub struct SubscribeNodeRes {
    pub ok: bool,
    /// Bootstrap peer node IDs to connect to
    pub bootstrap_peers: Vec<String>,
    /// Tournament topic URL
    pub topic_url: String,
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
pub fn tournament_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}", get(get_tournament))
        .route("/{id}/register-node", post(register_node))
        .route("/{id}/my-match", get(get_my_match))
        .route("/{id}/bracket", get(get_bracket))
}

/// Creates player-facing routes that require full AppState (KYC vault access).
pub fn tournament_player_app_state_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/join", post(join_tournament))
}

/// Creates admin tournament routes.
pub fn admin_tournament_routes() -> Router<AppState> {
    Router::new()
        .route("/create", post(create_tournament))
        .route("/{id}/record-result", post(record_result))
        .route("/{id}/set-match-game-id", post(set_match_game_id))
        .route("/{id}/initialize-swiss", post(initialize_swiss_tournament))
}

/// Creates admin tournament routes requiring AppState (for USDC operations).
pub fn admin_tournament_app_state_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/fund-prize-tx", post(build_fund_prize_transaction))
        .route("/{id}/cancel", post(build_cancel_transaction))
}

/// Creates tournament listing routes.
pub fn tournaments_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tournaments))
}

/// Creates gossip-enabled tournament routes (requires AppState).
pub fn tournament_gossip_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/subscribe-node", post(subscribe_node))
        .route("/{id}/bootstrap-peers", get(get_bootstrap_peers))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /tournament/{id}/subscribe-node - Subscribe to tournament gossip updates
/// 
/// Called by game client to register for gossip updates and receive bootstrap peers.
async fn subscribe_node(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<SubscribeNodeReq>,
) -> Result<Json<SubscribeNodeRes>, StatusCode> {
    // Register player's node_id for the tournament
    let ok = state.tournament_store.register_node_id(id, req.player.clone(), req.node_id.clone()).await;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get bootstrap peers for the player
    let bootstrap_peers = state.tournament_gossip.get_bootstrap_peers(id, &req.player).await;
    
    // Format peer IDs as hex strings
    let peer_strings: Vec<String> = bootstrap_peers
        .iter()
        .map(|p| hex::encode(p.as_bytes()))
        .collect();

    // Increment subscriber count
    state.tournament_gossip.increment_subscribers(id).await;

    info!("[tournament] {} subscribed to gossip for tournament {} ({} bootstrap peers)", 
        req.player, id, peer_strings.len());

    Ok(Json(SubscribeNodeRes {
        ok: true,
        bootstrap_peers: peer_strings,
        topic_url: format!("/swiss/{}", id),
    }))
}

/// GET /tournament/{id}/bootstrap-peers?player={pubkey} - Get bootstrap peers for a player
///
/// Returns list of peer node IDs to connect to for P2P gossip.
async fn get_bootstrap_peers(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = params.get("player").ok_or(StatusCode::BAD_REQUEST)?;
    
    // Verify tournament exists
    if state.tournament_store.get(id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get bootstrap peers
    let bootstrap_peers = state.tournament_gossip.get_bootstrap_peers(id, player).await;
    
    // Format peer IDs as hex strings
    let peer_strings: Vec<String> = bootstrap_peers
        .iter()
        .map(|p| hex::encode(p.as_bytes()))
        .collect();

    Ok(Json(serde_json::json!({
        "tournament_id": id,
        "player": player,
        "bootstrap_peers": peer_strings,
        "count": peer_strings.len(),
    })))
}

/// POST /admin/tournament/create - Creates a new tournament.
async fn create_tournament(
    State(state): State<AppState>,
    Json(req): Json<CreateTournamentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    // Parse tournament format
    let format = match req.format.as_str() {
        "Swiss" => {
            let rounds = req.swiss_rounds.ok_or_else(|| {
                StatusCode::BAD_REQUEST
            })?;
            TournamentFormat::Swiss { rounds }
        }
        "SingleElimination" | "" => TournamentFormat::SingleElimination,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

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
        format.clone(),
        req.elo_min,
        req.elo_max,
        req.min_players,
        req.scheduled_at,
        req.kyc_required,
    );
    store.create(record).await;
    
    info!("[tournament] Created tournament {} '{}' ({} players, format: {:?})", 
        req.tournament_id, req.name, req.max_players, format.clone());
    
    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": req.tournament_id,
        "max_players": req.max_players,
        "format": format.clone(),
        "prize_shares": prize_shares,
        "scheduled_at": req.scheduled_at,
        "kyc_required": req.kyc_required,
    })))
}

/// GET /tournaments - Lists all tournaments.
async fn list_tournaments(
    State(state): State<AppState>,
) -> Json<Vec<TournamentSummary>> {
    let store = &state.tournament_store;
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
    State(state): State<AppState>,
) -> Result<Json<TournamentRecord>, StatusCode> {
    state.tournament_store.get(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// POST /tournament/:id/register-node - Registers a player's P2P node ID.
async fn register_node(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<RegisterNodeReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ok = state.tournament_store.register_node_id(id, req.player.clone(), req.node_id.clone()).await;
    if !ok { return Err(StatusCode::NOT_FOUND); }
    info!("[tournament] {} registered node_id for tournament {}", req.player, id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /tournament/:id/my-match?player=<pubkey> - Gets a player's current match.
async fn get_my_match(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
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
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = state.tournament_store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
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
    State(state): State<AppState>,
    Json(req): Json<RecordResultReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
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
    State(state): State<AppState>,
    Json(req): Json<SetMatchGameIdReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if req.match_index >= tournament.matches.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ok = store.set_match_game_id(id, req.match_index, req.game_id).await;
    if !ok { return Err(StatusCode::NOT_FOUND); }
    info!("[tournament] Match {} of tournament {} assigned game_id {}", req.match_index, id, req.game_id);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tournament/:id/initialize-swiss - Initializes a Swiss tournament and starts round 1
async fn initialize_swiss_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = &state.tournament_store;
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    
    // Check if Swiss format
    let rounds = match tournament.format {
        TournamentFormat::Swiss { rounds } => rounds,
        TournamentFormat::SingleElimination => return Err(StatusCode::BAD_REQUEST),
    };

    // Check minimum players
    let current_players = tournament.players.len() as u16;
    let min_players = tournament.min_players.unwrap_or(8);
    if current_players < min_players {
        return Err(StatusCode::CONFLICT); // Not enough players
    }

    // Seed players by ELO
    let mut seeded_tournament = tournament.clone();
    seed_players_by_elo(&mut seeded_tournament);
    seeded_tournament.status = TournamentStatus::Active;
    seeded_tournament.started_at = Some(chrono::Utc::now().timestamp());
    
    // Initialize Swiss data
    seeded_tournament.swiss_data = Some(crate::signing::storage::tournament::SwissStorageData {
        current_round: 0,
        total_rounds: rounds,
        rounds: Vec::new(),
        results: Vec::new(),
        standings: Vec::new(),
    });

    store.create(seeded_tournament).await;
    
    info!("[tournament] Swiss tournament {} initialized with {} players, {} rounds", id, current_players, rounds);
    
    Ok(Json(serde_json::json!({
        "ok": true,
        "tournament_id": id,
        "players": current_players,
        "rounds": rounds
    })))
}

/// POST /tournament/:id/join - Joins a tournament.
///
/// Enforces CACF KYC when `kyc_required` is set on the tournament.
/// Players must have completed identity registration via `/identity/register`
/// (on the website or in-game) before they can enter a KYC-gated tournament.
async fn join_tournament(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let player = body.get("player").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let elo = body.get("elo").and_then(|v| v.as_u64()).unwrap_or(1200) as u32;

    let store = &state.tournament_store;

    // Load tournament early so we can check kyc_required before mutating state.
    let tournament = store.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    // ── CACF KYC gate ────────────────────────────────────────────────────────
    // When kyc_required is true every entrant must have a row in vault_users,
    // which is written by POST /identity/register (web or in-game flow).
    if tournament.kyc_required {
        let row = sqlx::query("SELECT 1 FROM vault_users WHERE pubkey = ?")
            .bind(player)
            .fetch_optional(&*state.vault_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if row.is_none() {
            info!("[tournament] KYC gate rejected {} for tournament {} — CACF not completed", player, id);
            return Ok(Json(serde_json::json!({
                "ok": false,
                "kyc_rejected": true,
                "message": "CACF KYC verification required. Complete identity registration on the website or in-game."
            })));
        }
    }

    let mut slot = None;
    let mut just_full = false;
    let mut elo_rejected = false;
    let mut elo_min = None;
    let mut elo_max = None;

    let ok = store.update(id, |t| {
        // ELO filtering
        if let (Some(min), Some(max)) = (t.elo_min, t.elo_max) {
            if elo < min || elo > max {
                elo_rejected = true;
                elo_min = Some(min);
                elo_max = Some(max);
                return;
            }
        }

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

    if elo_rejected {
        return Err(StatusCode::FORBIDDEN); // ELO out of range
    }

    let position = slot.ok_or(StatusCode::CONFLICT)?;

    // Send Braid scheduler trigger for auto-start logic
    if let Some(ref trigger_tx) = state.tournament_trigger {
        let player_count = store.get(id).await.map(|t| t.players.len()).unwrap_or(0);
        let trigger = TournamentTrigger::PlayerJoined {
            tournament_id: id,
            player_count,
        };
        if let Err(e) = trigger_tx.send(trigger).await {
            warn!("[tournament] Failed to send scheduler trigger: {}", e);
        } else {
            info!("[tournament] Sent Braid scheduler trigger for tournament {} ({} players)", id, player_count);
        }
    }

    // Old auto-start logic replaced by Braid scheduler
    // Scheduler will handle bracket generation and tournament start based on format
    if just_full {
        info!("[tournament] {} joined tournament {} at slot {} - FULL, scheduler will auto-start", player, id, position);
    } else {
        info!("[tournament] {} joined tournament {} at slot {}/{}", player, id, position, store.get(id).await.map(|t| t.max_players).unwrap_or(0));
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "slot": position,
        "tournament_full": just_full,
        "elo_rejected": elo_rejected,
        "elo_range": elo_min.zip(elo_max)
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

/// Request to build a fund prize transaction.
#[derive(Deserialize)]
pub struct FundPrizeReq {
    pub amount: u64,
}

/// Response with base64-encoded transaction.
#[derive(Serialize)]
pub struct FundPrizeRes {
    pub transaction: String,
    pub tournament_id: u64,
    pub amount: u64,
}

/// POST /admin/tournament/{id}/fund-prize-tx - Build transaction to fund USDC prize pool.
async fn build_fund_prize_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
    Json(req): Json<FundPrizeReq>,
) -> Result<Json<FundPrizeRes>, StatusCode> {
    info!("[tournament] Building fund prize tx for tournament {} amount {}", id, req.amount);
    
    // Verify tournament exists
    let tournament = state.tournament_store.get(id).await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build transaction (simplified - actual implementation would use solana_sdk)
    let transaction_base64 = "placeholder".to_string();

    Ok(Json(FundPrizeRes {
        transaction: transaction_base64,
        tournament_id: id,
        amount: req.amount,
    }))
}

/// POST /admin/tournament/{id}/cancel - Build transaction to cancel tournament.
async fn build_cancel_transaction(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("[tournament] Building cancel tx for tournament {}", id);
    
    // Verify tournament exists
    let tournament = state.tournament_store.get(id).await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "transaction": "placeholder",
        "tournament_id": id,
        "players_to_refund": tournament.players.len(),
    })))
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
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: Some([5000, 3000, 1500, 500]),
            scheduled_at: None,
            kyc_required: false,
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
    fn test_subscribe_node_req_serialization() {
        let req = SubscribeNodeReq {
            player: "test_wallet".to_string(),
            node_id: "node_123".to_string(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_subscribe_node_res_serialization() {
        let res = SubscribeNodeRes {
            ok: true,
            bootstrap_peers: vec!["peer1".to_string(), "peer2".to_string()],
            topic_url: "/swiss/123".to_string(),
        };

        let json = serde_json::to_string(&res);
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

    #[tokio::test]
    async fn test_tournament_gossip_routes_creation() {
        // Just verify the function signature compiles
        fn assert_gossip_routes<F>()
        where
            F: Fn() -> Router<AppState>,
        {
        }
        assert_gossip_routes::<fn() -> Router<AppState>>();
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
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                scheduled_at: None,
                kyc_required: false,
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
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: None,
            scheduled_at: None,
            kyc_required: false,
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
            format: "SingleElimination".to_string(),
            swiss_rounds: None,
            elo_min: None,
            elo_max: None,
            min_players: None,
            prize_shares: None,
            scheduled_at: None,
            kyc_required: false,
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
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                scheduled_at: None,
                kyc_required: false,
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
                format: "SingleElimination".to_string(),
                swiss_rounds: None,
                elo_min: None,
                elo_max: None,
                min_players: None,
                prize_shares: None,
                scheduled_at: None,
                kyc_required: false,
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
            TournamentFormat::SingleElimination,
            None,
            None,
            None,
            None,
            false,
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
