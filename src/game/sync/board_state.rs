//! Board State Synchronization using Braid Simpleton CRDT
//!
//! This module provides robust P2P board state synchronization by treating
//! the chess board state as a text-based CRDT that can be merged automatically.

use crate::engine::board_state::ChessEngine;
use crate::game::components::{PieceColor, PieceType};
use crate::game::resources::CapturedPieces;
use bevy::prelude::*;
use braid_core::core::merge::simpleton::SimpletonMergeType;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Resource for synchronizing board state between peers
#[derive(Resource)]
pub struct BoardStateSync {
    /// Simpleton CRDT for automatic conflict resolution
    pub merge_type: SimpletonMergeType,
    /// Last serialized state we know about
    pub last_known_state: String,
    /// Our peer ID for identification
    pub peer_id: String,
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
    /// Remote changes to apply
    PendingRemote,
    /// Conflict detected, resolving
    Resolving,
    /// Error state
    Error(String),
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

/// Serialized board state format:
/// FEN|move_counter|turn|last_move_from|last_move_to|captured_white|captured_black|hash
/// Example:
/// rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR|1|white|4|1|4|3||+|a3f2b8c...
#[derive(Debug, Clone)]
pub struct SerializedBoardState {
    pub fen: String,
    pub move_counter: u32,
    pub current_turn: PieceColor,
    pub last_move: Option<((u8, u8), (u8, u8))>,
    pub captured_white: Vec<PieceType>,
    pub captured_black: Vec<PieceType>,
    pub hash: String,
}

impl BoardStateSync {
    /// Create a new BoardStateSync with a unique peer ID
    pub fn new(peer_id: &str) -> Self {
        Self {
            merge_type: SimpletonMergeType::new(peer_id),
            last_known_state: String::new(),
            peer_id: peer_id.to_string(),
            pending_moves: Vec::new(),
            sync_status: SyncStatus::Initializing,
        }
    }

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

    /// Parse a serialized board state string
    pub fn deserialize_state(state_str: &str) -> Result<SerializedBoardState, SyncError> {
        let parts: Vec<&str> = state_str.split('|').collect();
        if parts.len() != 8 {
            return Err(SyncError::InvalidFormat(format!(
                "Expected 8 parts, got {}",
                parts.len()
            )));
        }

        let fen = parts[0].to_string();
        let move_counter = parts[1]
            .parse::<u32>()
            .map_err(|e| SyncError::InvalidFormat(format!("Invalid move_counter: {}", e)))?;
        let current_turn = match parts[2] {
            "w" => PieceColor::White,
            "b" => PieceColor::Black,
            _ => {
                return Err(SyncError::InvalidFormat(format!(
                    "Invalid turn: {}",
                    parts[2]
                )))
            }
        };

        // Parse last move
        let last_move = if parts[3].is_empty() {
            None
        } else {
            let from_x = parts[3]
                .parse::<u8>()
                .map_err(|_| SyncError::InvalidFormat("Invalid from_x".to_string()))?;
            let from_y = parts[4]
                .parse::<u8>()
                .map_err(|_| SyncError::InvalidFormat("Invalid from_y".to_string()))?;
            let to_x = parts[5]
                .parse::<u8>()
                .map_err(|_| SyncError::InvalidFormat("Invalid to_x".to_string()))?;
            let to_y = parts[6]
                .parse::<u8>()
                .map_err(|_| SyncError::InvalidFormat("Invalid to_y".to_string()))?;
            Some(((from_x, from_y), (to_x, to_y)))
        };

        // Parse captured pieces
        let captured_white = parts[5].chars().filter_map(char_to_piece_type).collect();
        let captured_black = parts[6].chars().filter_map(char_to_piece_type).collect();

        // Verify hash
        let content = format!(
            "{}|{}|{}|{}|{}|{}",
            fen, move_counter, parts[2], parts[3], parts[4], parts[5]
        );
        let expected_hash = calculate_state_hash(&content);
        if expected_hash != parts[7] {
            return Err(SyncError::HashMismatch);
        }

        Ok(SerializedBoardState {
            fen,
            move_counter,
            current_turn,
            last_move,
            captured_white,
            captured_black,
            hash: parts[7].to_string(),
        })
    }

    /// Apply a remote state to the local game
    pub fn apply_remote_state(
        &mut self,
        state_str: &str,
        current_move_counter: u32,
    ) -> Result<StateDiff, SyncError> {
        let remote_state = Self::deserialize_state(state_str)?;

        // Determine what to do based on move counter
        match remote_state.move_counter.cmp(&current_move_counter) {
            std::cmp::Ordering::Less => {
                // Remote is behind - ignore or help them catch up
                self.sync_status = SyncStatus::PendingLocal;
                Ok(StateDiff::NoAction)
            }
            std::cmp::Ordering::Equal => {
                // Same move count - check if states match
                if self.last_known_state == state_str {
                    self.sync_status = SyncStatus::Synchronized;
                    Ok(StateDiff::NoAction)
                } else {
                    // Different states at same move - conflict!
                    self.sync_status = SyncStatus::Resolving;
                    Err(SyncError::Conflict)
                }
            }
            std::cmp::Ordering::Greater => {
                // Remote is ahead - apply their state
                self.sync_status = SyncStatus::PendingRemote;
                Ok(StateDiff::ApplyState(remote_state))
            }
        }
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

/// Difference between local and remote state
#[derive(Debug)]
pub enum StateDiff {
    /// No action needed
    NoAction,
    /// Apply this remote state
    ApplyState(SerializedBoardState),
    /// Replay these moves
    ReplayMoves(Vec<BoardMove>),
}

/// Errors that can occur during sync
#[derive(Debug, Clone)]
pub enum SyncError {
    InvalidFormat(String),
    HashMismatch,
    InvalidFen,
    Conflict,
    EngineError(String),
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

/// Convert character to PieceType
fn char_to_piece_type(c: char) -> Option<PieceType> {
    match c {
        'K' => Some(PieceType::King),
        'Q' => Some(PieceType::Queen),
        'R' => Some(PieceType::Rook),
        'B' => Some(PieceType::Bishop),
        'N' => Some(PieceType::Knight),
        'P' => Some(PieceType::Pawn),
        _ => None,
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

        // Update simpleton merge type
        board_sync.merge_type.content = state.clone();

        // TODO: Write to braid-blob file or send via braid-iroh
        // For now, just update the state
        board_sync.on_sync_complete(state);
    }
}

/// System to receive and apply remote state
pub fn receive_state_system(
    mut board_sync: ResMut<BoardStateSync>,
    mut engine: ResMut<ChessEngine>,
    // Add braid network reader here when integrated
) {
    // TODO: Read from braid-blob file or receive via braid-iroh
    // For now, placeholder

    // Example:
    // if let Some(remote_state) = receive_from_network() {
    //     match board_sync.apply_remote_state(&remote_state, engine.get_move_counter()) {
    //         Ok(StateDiff::ApplyState(state)) => {
    //             engine.import_fen(&state.fen);
    //             // Update captured pieces
    //             // Apply visual updates
    //         }
    //         Ok(StateDiff::NoAction) => {}
    //         Ok(StateDiff::ReplayMoves(moves)) => {
    //             // Replay moves one by one
    //         }
    //         Err(e) => {
    //             board_sync.sync_status = SyncStatus::Error(format!("{:?}", e));
    //         }
    //     }
    // }
}

/// Initialize BoardStateSync on startup
pub fn init_board_state_sync(mut commands: Commands) {
    // Generate unique peer ID
    let peer_id = format!("peer_{}", rand::random::<u16>());
    commands.insert_resource(BoardStateSync::new(&peer_id));
    info!("[BOARD_SYNC] Initialized with peer ID: {}", peer_id);
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
