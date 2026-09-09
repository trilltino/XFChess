use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const LOBBY_TTL_SECS: i64 = 90;

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
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameStatus {
    Open,       // Waiting for opponent
    Connecting, // Someone trying to join
    InProgress, // Game started
    Finished,   // Game ended
}

impl Default for GameStatus {
    fn default() -> Self {
        GameStatus::Open
    }
}

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
    pub capacity: u8,
    pub players_joined: u8,
    pub ttl_seconds: i64,
    pub is_private: bool,
}

#[derive(Debug, Clone)]
pub struct ActiveGame {
    pub announcement: P2PGameAnnouncement,
    pub joiner_node_id: Option<String>,
    pub host_messages: Vec<String>, // JSON-serialized moves
    pub joiner_messages: Vec<String>,
    pub last_activity: DateTime<Utc>,
    pub pending_invites: Vec<String>,
}

// Request/Response Types

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
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AnnounceGameResponse {
    pub success: bool,
}

#[derive(Serialize, Deserialize)]
pub struct JoinGameRequest {
    pub game_id: String,
    pub joiner_node_id: String,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptJoinReq {
    pub game_id: String,
    pub host_node_id: String,
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
pub struct HeartbeatRequest {
    pub game_id: String,
    pub host_node_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub game_id: String,
    pub from_node_id: String,
    pub message: String, // JSON-serialized move
    pub signature: Vec<u8>,
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
