//! Data types for P2P relay.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Game announcement for P2P matchmaking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PGameAnnouncement {
    pub game_id: String,
    pub host_node_id: String, // Internal - not exposed in listing
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String, // "P2P" or "Solana"
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub created_at: DateTime<Utc>,
    pub status: GameStatus,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
}

/// Game status in the relay system
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

/// Public game listing (hides internal node IDs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameListing {
    pub game_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub status: GameStatus,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
}

/// Internal active game state
#[derive(Debug, Clone)]
pub struct ActiveGame {
    pub announcement: P2PGameAnnouncement,
    pub joiner_node_id: Option<String>,
    pub host_messages: Vec<String>, // JSON-serialized moves
    pub joiner_messages: Vec<String>,
    pub last_activity: DateTime<Utc>,
}

// Request/Response Types

/// Request to announce a new P2P game
#[derive(Serialize, Deserialize)]
pub struct AnnounceGameRequest {
    pub game_id: String,
    pub host_node_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
}

/// Response to game announcement
#[derive(Serialize, Deserialize)]
pub struct AnnounceGameResponse {
    pub success: bool,
}

/// Request to join a P2P game
#[derive(Serialize, Deserialize)]
pub struct JoinGameRequest {
    pub game_id: String,
    pub joiner_node_id: String,
}

/// Response to join request
#[derive(Serialize, Deserialize)]
pub struct JoinGameResponse {
    pub success: bool,
    pub host_node_id: Option<String>, // Revealed only to joiner
}

/// Request to leave a P2P game
#[derive(Serialize, Deserialize)]
pub struct LeaveGameRequest {
    pub game_id: String,
    pub node_id: String,
}

/// Request to send a message in a game
#[derive(Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub game_id: String,
    pub from_node_id: String,
    pub message: String, // JSON-serialized move
}

/// Request to poll for new messages
#[derive(Serialize, Deserialize)]
pub struct PollMessagesRequest {
    pub game_id: String,
    pub node_id: String,
    pub since_index: usize,
}

/// Response to poll request
#[derive(Serialize, Deserialize)]
pub struct PollMessagesResponse {
    pub messages: Vec<String>,
    pub next_index: usize,
}
