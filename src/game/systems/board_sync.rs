//! FastBoardState synchronization system
//!
//! Keeps the bitboard representation in sync with ECS piece entities.
//! Rebuilds the bitboard when pieces are added, removed, or moved.

use bevy::prelude::*;
use crate::rendering::pieces::{Piece, PieceColor};
use crate::game::resources::FastBoardState;

/// System that synchronizes FastBoardState with ECS piece positions
///
/// Runs in the Validation set, before move validation systems that need
/// the fast board state for O(1) lookups.
///
/// Automatically rebuilds when piece count changes (pieces added/removed)
/// or when explicitly marked dirty (pieces moved).
pub fn sync_fast_board_state(
    mut fast_board: ResMut<FastBoardState>,
    pieces: Query<&Piece>,
) {
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
