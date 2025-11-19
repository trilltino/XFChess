//! FastBoardState synchronization system
//!
//! Keeps the bitboard representation in sync with ECS piece entities.
//! Rebuilds the bitboard when pieces are added, removed, or moved.
//!
//! # Performance
//!
//! This system uses change detection to avoid unnecessary rebuilds:
//! - Only rebuilds when piece count changes (add/remove)
//! - Only rebuilds when explicitly marked dirty (moved)
//! - Provides O(1) square occupancy checks via bitboards
//!
//! # Execution Order
//!
//! Runs in `GameSystems::Validation` set, before move validation systems
//! that need the fast board state for O(1) lookups.

use crate::game::resources::FastBoardState;
use crate::rendering::pieces::{Piece, PieceColor};
use bevy::prelude::*;

/// System that synchronizes FastBoardState with ECS piece positions
///
/// Rebuilds the bitboard representation from ECS piece entities, enabling
/// O(1) square occupancy checks for move validation.
///
/// # Execution Order
///
/// Runs in `GameSystems::Validation` set, before move validation systems
/// that need the fast board state.
///
/// # Rebuild Conditions
///
/// Automatically rebuilds when:
/// - Piece count changes (pieces added/removed)
/// - Board is explicitly marked dirty (pieces moved)
///
/// # Performance
///
/// Uses early return to skip rebuilds when board is already in sync,
/// minimizing unnecessary work each frame.
pub fn sync_fast_board_state(mut fast_board: ResMut<FastBoardState>, pieces: Query<&Piece>) {
    // Always rebuild if piece count changed (add/remove) or if marked dirty (moved)
    let piece_count = pieces.iter().count() as u32;
    let current_count = fast_board.piece_count();

    if !fast_board.dirty && piece_count == current_count {
        return; // Board is in sync
    }

    // Clear and rebuild
    fast_board.clear();

    for piece in pieces.iter() {
        match piece.color {
            PieceColor::White => fast_board.set_white(piece.x, piece.y),
            PieceColor::Black => fast_board.set_black(piece.x, piece.y),
        }
    }

    fast_board.dirty = false;

    // Debug logging (only when rebuild happens)
    debug!(
        "[BOARD_SYNC] Rebuilt FastBoardState: {} pieces (W:{} B:{})",
        fast_board.piece_count(),
        fast_board.white_count(),
        fast_board.black_count()
    );
}
