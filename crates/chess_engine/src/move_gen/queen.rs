//! Queen move generation
//!
//! Handles queen-specific move generation. Queens combine the movement patterns
//! of bishops and rooks, making them the most powerful piece on the board.
//!
//! ## Queen Movement Rules
//!
//! - Queens move like bishops (diagonally) OR rooks (horizontally/vertically)
//! - Cannot jump over pieces
//! - Cannot move to squares occupied by own pieces
//! - Can capture opponent pieces on destination squares
//! - Movement stops when blocked by any piece

use super::bishop;
use super::rook;
use crate::types::*;

/// Generate queen moves from a given square
///
/// Queens combine bishop and rook movement, so this function generates
/// moves for both patterns and combines them.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the queen (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_queen_moves(&game, 3, COLOR_WHITE, &mut moves);
/// // Moves now contains all valid queen moves from d1 (diagonal + horizontal/vertical)
/// ```
pub fn generate_queen_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    // Queen moves like both bishop and rook
    bishop::generate_bishop_moves(game, from, color, moves);
    rook::generate_rook_moves(game, from, color, moves);
}
