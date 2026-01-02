//! Knight move generation
//!
//! Handles knight-specific move generation. Knights move in an L-shape pattern:
//! 2 squares in one direction, then 1 square perpendicular (or vice versa).
//!
//! ## Knight Movement Rules
//!
//! - Knights can jump over pieces (unlike sliding pieces)
//! - 8 possible destinations from most squares (fewer near edges)
//! - Cannot move to squares occupied by own pieces
//! - Can capture opponent pieces on destination squares

use crate::board::*;
use crate::types::*;

/// Generate knight moves from a given square
///
/// This function filters the precomputed knight move table, removing moves
/// that would land on squares occupied by own pieces.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the knight (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_knight_moves(&game, 1, COLOR_WHITE, &mut moves);
/// // Moves now contains all valid knight moves from b1
/// ```
pub fn generate_knight_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    // Knights can jump over pieces, so we just need to check destination squares
    for candidate in &game.knight[from as usize] {
        let dest_piece = game.board[candidate.dst as usize];

        // Valid if destination is empty or contains opponent piece
        if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
        }
    }
}
