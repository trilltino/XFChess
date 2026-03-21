use serde::{Deserialize, Serialize};

/// Game state messages compatible with backend gossip observer
/// Maps from ChessMessage (braid_uri) to backend-expected format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateMessage {
    pub message_type: String,
    pub game_id: String,
    pub player1: Option<String>,
    pub player2: Option<String>,
    pub stake_amount: Option<f64>,
    pub move_details: Option<String>,
    pub winner: Option<String>,
    pub timestamp: Option<u64>,
}

impl GameStateMessage {
    /// Create a game start message
    pub fn game_start(game_id: String, player1: String, player2: String, stake_amount: f64) -> Self {
        Self {
            message_type: "game_start".to_string(),
            game_id,
            player1: Some(player1),
            player2: Some(player2),
            stake_amount: Some(stake_amount),
            move_details: None,
            winner: None,
            timestamp: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        }
    }

    /// Create a move made message
    pub fn move_made(game_id: String, move_uci: String) -> Self {
        Self {
            message_type: "move_made".to_string(),
            game_id,
            player1: None,
            player2: None,
            stake_amount: None,
            move_details: Some(move_uci),
            winner: None,
            timestamp: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        }
    }

    /// Create a game end message
    pub fn game_end(game_id: String, winner: String) -> Self {
        Self {
            message_type: "game_end".to_string(),
            game_id,
            player1: None,
            player2: None,
            stake_amount: None,
            move_details: None,
            winner: Some(winner),
            timestamp: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        }
    }
}

/// Board state representation for backend compatibility
/// Re-exports from multiplayer module to avoid circular dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardState {
    pub fen: String,
    pub turn: String,
    pub move_number: u32,
    pub status: String,
}

impl Default for BoardState {
    fn default() -> Self {
        Self {
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            turn: "white".to_string(),
            move_number: 1,
            status: "playing".to_string(),
        }
    }
}
