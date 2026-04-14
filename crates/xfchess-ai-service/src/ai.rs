use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRequest {
    pub fen: String,
    pub player_side: String, // "white" or "black"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveResponse {
    pub best_move: String,
    pub evaluation: i32,
    pub depth: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Simple AI logic - placeholder for Stockfish integration
pub fn get_best_move(_fen: &str, _player_side: &str) -> MoveResponse {
    // This would integrate with Stockfish in a real implementation
    // For now, return a placeholder response
    MoveResponse {
        best_move: "e2e4".to_string(), // Placeholder move
        evaluation: 0,
        depth: 15,
    }
}
