//! King move generation
//!
//! Handles king-specific move generation. Kings move one square in any direction
//! (horizontally, vertically, or diagonally).
//!
//! ## King Movement Rules
//!
//! - Kings move one square in any direction (8 possible destinations)
//! - Cannot move to squares occupied by own pieces
//! - Cannot move to squares attacked by opponent pieces (handled during validation)
//! - Can capture opponent pieces on destination squares
//! - Castling is handled separately (not in move tables)
//!
//! ## Note on Castling
//!
//! Castling is not included in the precomputed move tables because it depends on:
//! - King and rook not having moved
//! - No pieces between king and rook
//! - King not in check
//! - Squares not under attack
//!
//! These conditions are checked during move generation/validation.

use crate::board::*;
use crate::types::*;

/// Generate king moves from a given square
///
/// This function filters the precomputed king move table, removing moves
/// that would land on squares occupied by own pieces.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the king (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_king_moves(&game, 4, COLOR_WHITE, &mut moves);
/// // Moves now contains all valid king moves from e1 (excluding castling)
/// ```
pub fn generate_king_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    // Kings move one square, so we just need to check destination squares
    for candidate in &game.king[from as usize] {
        let dest_piece = game.board[candidate.dst as usize];

        // Valid if destination is empty or contains opponent piece
        if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
        }
    }

    // Note: Castling would be handled here in a full implementation
    // by checking castling rights and adding castling moves if valid
}
