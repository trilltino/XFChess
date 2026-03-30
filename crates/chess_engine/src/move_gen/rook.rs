//! Rook move generation
//!
//! Handles rook-specific move generation. Rooks are sliding pieces that move
//! horizontally and vertically until blocked by another piece or the board edge.
//!
//! ## Rook Movement Rules
//!
//! - Rooks move horizontally (along ranks) or vertically (along files)
//! - Cannot jump over pieces
//! - Cannot move to squares occupied by own pieces
//! - Can capture opponent pieces on destination squares
//! - Movement stops when blocked by any piece

use super::sliding;
use crate::types::*;

/// Generate rook moves from a given square
///
/// This function delegates to the common sliding piece logic, using the
/// precomputed rook move table for this square.
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the rook (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_rook_moves(&game, 0, COLOR_WHITE, &mut moves);
/// // Moves now contains all valid rook moves from a1
/// ```
pub fn generate_rook_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    sliding::generate_sliding_moves(game, from, color, &game.rook[from as usize], moves);
}
