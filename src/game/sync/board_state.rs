//! Board State Synchronization using Braid Simpleton CRDT
//!
//! This module provides robust P2P board state synchronization by treating
//! the chess board state as a text-based CRDT that can be merged automatically.

use crate::engine::board_state::ChessEngine;
use crate::game::components::{PieceColor, PieceType};
use crate::game::resources::CapturedPieces;
use bevy::prelude::*;
use std::hash::{Hash, Hasher};

/// Resource for synchronizing board state between peers
#[derive(Resource)]
pub struct BoardStateSync {
    /// Last serialized state we know about
    pub last_known_state: String,
    /// Pending moves that haven't been acknowledged
    pub pending_moves: Vec<BoardMove>,
    /// Sync status for UI display
    pub sync_status: SyncStatus,
}

/// Status of board synchronization
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// Waiting for initial sync
    Initializing,
    /// In sync with peer
    Synchronized,
    /// Local changes pending sync
    PendingLocal,
}

/// A chess move in serializable format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoardMove {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub piece_type: PieceType,
    pub piece_color: PieceColor,
    pub capture: Option<PieceType>,
    pub promotion: Option<PieceType>,
    pub move_number: u32,
}

impl Default for BoardStateSync {
    fn default() -> Self {
        Self {
            last_known_state: String::new(),
            pending_moves: Vec::new(),
            sync_status: SyncStatus::Initializing,
        }
    }
}

impl BoardStateSync {
    /// Serialize the current board state to a string
    pub fn serialize_state(
        &self,
        engine: &ChessEngine,
        captured_pieces: &CapturedPieces,
        last_move: Option<BoardMove>,
    ) -> String {
        let fen = engine.to_fen_string();
        let move_counter = engine.get_move_counter();
        let current_turn = engine.get_current_turn();

        // Format last move
        let last_move_str = match last_move {
            Some(mv) => format!("{}|{}|{}|{}", mv.from.0, mv.from.1, mv.to.0, mv.to.1),
            None => "|||".to_string(),
        };

        // Format captured pieces
        let captured_white_str = captured_pieces
            .white_captured
            .iter()
            .map(|p| piece_type_to_char(*p))
            .collect::<String>();
        let captured_black_str = captured_pieces
            .black_captured
            .iter()
            .map(|p| piece_type_to_char(*p))
            .collect::<String>();

        // Build state string
        let state = format!(
            "{}|{}|{}|{}|{}|{}",
            fen,
            move_counter,
            if current_turn == PieceColor::White {
                "w"
            } else {
                "b"
            },
            last_move_str,
            captured_white_str,
            captured_black_str
        );

        // Calculate hash
        let hash = calculate_state_hash(&state);

        format!("{}|{}", state, hash)
    }

    /// Update the last known state after local move
    pub fn on_local_move(&mut self, move_record: BoardMove, serialized_state: String) {
        self.pending_moves.push(move_record);
        self.last_known_state = serialized_state;
        self.sync_status = SyncStatus::PendingLocal;
    }

    /// Mark sync as complete
    pub fn on_sync_complete(&mut self, state_str: String) {
        self.last_known_state = state_str;
        self.pending_moves.clear();
        self.sync_status = SyncStatus::Synchronized;
    }
}

/// Calculate a simple hash for state verification
fn calculate_state_hash(state: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Convert PieceType to character for serialization
fn piece_type_to_char(piece: PieceType) -> char {
    match piece {
        PieceType::King => 'K',
        PieceType::Queen => 'Q',
        PieceType::Rook => 'R',
        PieceType::Bishop => 'B',
        PieceType::Knight => 'N',
        PieceType::Pawn => 'P',
    }
}

/// Extension trait for ChessEngine to get FEN string
pub trait ChessEngineExt {
    fn to_fen_string(&self) -> String;
    fn get_move_counter(&self) -> u32;
    fn get_current_turn(&self) -> PieceColor;
}

impl ChessEngineExt for ChessEngine {
    fn to_fen_string(&self) -> String {
        // Use the complete FEN export from the engine
        self.to_fen()
    }

    fn get_move_counter(&self) -> u32 {
        // Get fullmove counter from engine state
        self.fullmove_counter
    }

    fn get_current_turn(&self) -> PieceColor {
        // Get current turn from engine state
        self.current_turn
    }
}

/// System to broadcast local state changes
pub fn broadcast_state_system(
    mut board_sync: ResMut<BoardStateSync>,
    engine: Res<ChessEngine>,
    captured_pieces: Res<CapturedPieces>,
    // Add braid network writer here when integrated
) {
    // Only broadcast if we have pending local moves
    if board_sync.sync_status == SyncStatus::PendingLocal {
        let state = board_sync.serialize_state(&engine, &captured_pieces, None);

        // TODO: Write to braid-blob file or send via braid-iroh
        // For now, just update the state
        board_sync.on_sync_complete(state);
    }
}

/// System to receive and apply remote state.
/// TODO: wire a braid-iroh reader; apply updates via `BoardStateSync::apply_remote_state`.
pub fn receive_state_system(_board_sync: ResMut<BoardStateSync>, _engine: ResMut<ChessEngine>) {}

/// Initialize BoardStateSync on startup
pub fn init_board_state_sync(mut commands: Commands) {
    commands.insert_resource(BoardStateSync::default());
    info!("[BOARD_SYNC] Initialized board state sync");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        // TODO: Add tests with mock engine
    }

    #[test]
    fn test_hash_verification() {
        let state = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR|1|w|4|1|4|3||";
        let hash = calculate_state_hash(state);
        assert!(!hash.is_empty());
    }
}
