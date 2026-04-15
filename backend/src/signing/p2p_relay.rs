//! P2P game relay for VPS-mediated peer-to-peer connections.
//!
//! This module provides a VPS-mediated relay system for P2P chess games,
//! enabling reliable NAT traversal for players behind firewalls.
//!
//! Features:
//! - Game announcement and discovery
//! - Player matchmaking with node ID exchange
//! - In-game message relay for move data
//! - Automatic cleanup of stale games

use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use super::AppState;

// ── Data Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PGameAnnouncement {
    pub game_id: String,
    pub host_node_id: String, // Internal - not exposed in listing
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String, // "P2P" or "Solana"
    pub time_control_minutes: u32,
    pub created_at: DateTime<Utc>,
    pub status: GameStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameStatus {
    Open,       // Waiting for opponent
    Connecting, // Someone trying to join
    InProgress, // Game started
    Finished,   // Game ended
}

impl Default for GameStatus {
    fn default() -> Self { GameStatus::Open }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameListing {
    pub game_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub time_control_minutes: u32,
    pub status: GameStatus,
}

#[derive(Debug, Clone)]
pub struct ActiveGame {
    pub announcement: P2PGameAnnouncement,
    pub joiner_node_id: Option<String>,
    pub host_messages: Vec<String>, // JSON-serialized moves
    pub joiner_messages: Vec<String>,
    pub last_activity: DateTime<Utc>,
}

// ── State ───────────────────────────────────────────────────────────────────

pub type P2PRelayState = Arc<RwLock<HashMap<String, ActiveGame>>>;

pub fn create_relay_state() -> P2PRelayState {
    let state: P2PRelayState = Arc::new(RwLock::new(HashMap::new()));
    
    // Spawn cleanup task
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            cleanup_stale_games(&state_clone);
        }
    });
    
    state
}

fn cleanup_stale_games(state: &P2PRelayState) {
    let mut games = state.write().expect("P2P relay mutex should not be poisoned");
    let now = Utc::now();
    let stale_threshold = chrono::Duration::minutes(5);
    
    games.retain(|_, game| {
        let elapsed = now.signed_duration_since(game.last_activity);
        let is_stale = elapsed > stale_threshold;
        let is_finished = game.announcement.status == GameStatus::Finished;
        
        if is_stale && !is_finished {
            tracing::info!("Cleaning up stale game {}", game.announcement.game_id);
        }
        
        !is_stale || is_finished
    });
}

// ── Request/Response Types ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct AnnounceGameRequest {
    pub game_id: String,
    pub host_node_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub time_control_minutes: u32,
}

#[derive(Serialize, Deserialize)]
pub struct AnnounceGameResponse {
    pub success: bool,
}

#[derive(Serialize, Deserialize)]
pub struct JoinGameRequest {
    pub game_id: String,
    pub joiner_node_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct JoinGameResponse {
    pub success: bool,
    pub host_node_id: Option<String>, // Revealed only to joiner
}

#[derive(Serialize, Deserialize)]
pub struct LeaveGameRequest {
    pub game_id: String,
    pub node_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub game_id: String,
    pub from_node_id: String,
    pub message: String, // JSON-serialized move
}

#[derive(Serialize, Deserialize)]
pub struct PollMessagesRequest {
    pub game_id: String,
    pub node_id: String,
    pub since_index: usize,
}

#[derive(Serialize, Deserialize)]
pub struct PollMessagesResponse {
    pub messages: Vec<String>,
    pub next_index: usize,
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn p2p_routes() -> Router<AppState> {
    Router::new()
        .route("/p2p/announce", post(announce_game))
        .route("/p2p/games", get(list_games))
        .route("/p2p/join", post(join_game))
        .route("/p2p/accept", post(accept_join))
        .route("/p2p/leave", post(leave_game))
        .route("/p2p/message", post(send_message))
        .route("/p2p/poll", post(poll_messages))
}

// ── Route Handlers ─────────────────────────────────────────────────────────

pub async fn announce_game(
    State(state): State<AppState>,
    Json(req): Json<AnnounceGameRequest>,
) -> Result<Json<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Clone before moving into struct
    let game_id_clone = req.game_id.clone();
    let host_node_id_clone = req.host_node_id.clone();
    
    let announcement = P2PGameAnnouncement {
        game_id: req.game_id,
        host_node_id: req.host_node_id,
        display_name: req.display_name,
        stake_amount: req.stake_amount,
        game_type: req.game_type,
        time_control_minutes: req.time_control_minutes,
        created_at: Utc::now(),
        status: GameStatus::Open,
    };
    
    let active_game = ActiveGame {
        announcement,
        joiner_node_id: None,
        host_messages: Vec::new(),
        joiner_messages: Vec::new(),
        last_activity: Utc::now(),
    };
    
    games.insert(game_id_clone.clone(), active_game);
    
    tracing::info!("P2P game announced: {} by {}", game_id_clone, host_node_id_clone);
    
    Ok(Json(AnnounceGameResponse { success: true }))
}

pub async fn list_games(
    State(state): State<AppState>,
) -> Result<Json<Vec<GameListing>>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let listings: Vec<GameListing> = games
        .values()
        .filter(|g| g.announcement.status == GameStatus::Open)
        .map(|g| GameListing {
            game_id: g.announcement.game_id.clone(),
            display_name: g.announcement.display_name.clone(),
            stake_amount: g.announcement.stake_amount,
            game_type: g.announcement.game_type.clone(),
            time_control_minutes: g.announcement.time_control_minutes,
            status: g.announcement.status.clone(),
        })
        .collect();
    
    Ok(Json(listings))
}

pub async fn join_game(
    State(state): State<AppState>,
    Json(req): Json<JoinGameRequest>,
) -> Result<Json<JoinGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let game_id_clone = req.game_id.clone();
    let joiner_node_id_clone = req.joiner_node_id.clone();
    
    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(Json(JoinGameResponse { success: false, host_node_id: None }));
    };
    
    if game.announcement.status != GameStatus::Open {
        return Ok(Json(JoinGameResponse { success: false, host_node_id: None }));
    }
    
    game.joiner_node_id = Some(req.joiner_node_id);
    game.announcement.status = GameStatus::Connecting;
    game.last_activity = Utc::now();
    
    let host_node_id = game.announcement.host_node_id.clone();
    
    tracing::info!(
        "P2P join request: game={}, joiner={}",
        game_id_clone,
        joiner_node_id_clone
    );
    
    Ok(Json(JoinGameResponse {
        success: true,
        host_node_id: Some(host_node_id),
    }))
}

pub async fn accept_join(
    State(state): State<AppState>,
    Json(req): Json<JoinGameRequest>, // game_id + host_node_id confirmation
) -> Result<Json<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let game_id_for_log = req.game_id.clone();
    
    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(Json(AnnounceGameResponse { success: false }));
    };
    
    if game.announcement.host_node_id != req.joiner_node_id {
        // Wrong host trying to accept
        return Ok(Json(AnnounceGameResponse { success: false }));
    }
    
    game.announcement.status = GameStatus::InProgress;
    game.last_activity = Utc::now();
    
    tracing::info!("P2P game {} started", game_id_for_log);
    
    Ok(Json(AnnounceGameResponse { success: true }))
}

pub async fn leave_game(
    State(state): State<AppState>,
    Json(req): Json<LeaveGameRequest>,
) -> Result<Json<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if let Some(game) = games.get_mut(&req.game_id) {
        if game.announcement.host_node_id == req.node_id {
            // Host left - remove game
            game.announcement.status = GameStatus::Finished;
            tracing::info!("P2P game {} ended (host left)", req.game_id);
        } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
            // Joiner left
            game.joiner_node_id = None;
            game.announcement.status = GameStatus::Open;
            tracing::info!("P2P game {} open again (joiner left)", req.game_id);
        }
    }
    
    Ok(Json(AnnounceGameResponse { success: true }))
}

pub async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<AnnounceGameResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let mut games = relay_state.write().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let Some(game) = games.get_mut(&req.game_id) else {
        return Ok(Json(AnnounceGameResponse { success: false }));
    };
    
    game.last_activity = Utc::now();
    
    if game.announcement.host_node_id == req.from_node_id {
        // Message from host to joiner
        game.host_messages.push(req.message);
    } else if game.joiner_node_id.as_ref() == Some(&req.from_node_id) {
        // Message from joiner to host
        game.joiner_messages.push(req.message);
    } else {
        return Ok(Json(AnnounceGameResponse { success: false }));
    }
    
    Ok(Json(AnnounceGameResponse { success: true }))
}

pub async fn poll_messages(
    State(state): State<AppState>,
    Json(req): Json<PollMessagesRequest>,
) -> Result<Json<PollMessagesResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let Some(game) = games.get(&req.game_id) else {
        return Ok(Json(PollMessagesResponse {
            messages: vec![],
            next_index: req.since_index,
        }));
    };
    
    let messages = if game.announcement.host_node_id == req.node_id {
        // Host polls joiner messages
        game.joiner_messages[req.since_index..].to_vec()
    } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
        // Joiner polls host messages
        game.host_messages[req.since_index..].to_vec()
    } else {
        vec![]
    };
    
    let next_index = req.since_index + messages.len();
    
    Ok(Json(PollMessagesResponse {
        messages,
        next_index,
    }))
}
