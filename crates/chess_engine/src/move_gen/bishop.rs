//! Bishop move generation
//!
//! Handles bishop-specific move generation. Bishops are sliding pieces that
//! move diagonally until blocked by another piece or the board edge.
//!
//! ## Bishop Movement Rules
//!
//! - Bishops move diagonally (any number of squares)
//! - Cannot jump over pieces
//! - Cannot move to squares occupied by own pieces
//! - Can capture opponent pieces on destination squares
//! - Movement stops when blocked by any piece

use super::sliding;
use crate::types::*;

/// Generate bishop moves from a given square
///
/// This function delegates to the common sliding piece logic, using the
/// precomputed bishop move table for this square.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the bishop (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_bishop_moves(&game, 2, COLOR_WHITE, &mut moves);
/// // Moves now contains all valid bishop moves from c1
/// ```
pub fn generate_bishop_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    sliding::generate_sliding_moves(game, from, color, &game.bishop[from as usize], moves);
}
