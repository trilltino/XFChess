//! Sliding piece move generation
//!
//! Common functionality for sliding pieces (bishops, rooks, queens).
//! These pieces can move multiple squares in a direction until blocked.
//!
//! ## Algorithm
//!
//! Sliding pieces use precomputed move tables that contain all possible
//! destinations along each direction. During move generation, we:
//! 1. Iterate through precomputed moves
//! 2. Stop when we hit a piece (blocked)
//! 3. Include the blocking square if it's an opponent piece (capture)
//! 4. Track blocked directions to avoid redundant checks
//!
//! ## Performance
//!
//! - **Time complexity**: O(n) where n is the number of moves in the table
//! - **Space complexity**: O(1) - uses precomputed tables
//! - **Typical moves per square**: 14 for rooks, 13 for bishops, 27 for queens

use crate::board::*;
use crate::types::*;

/// Generate moves for a sliding piece using a precomputed move table
///
/// This function handles the common logic for bishops, rooks, and queens.
/// It filters the precomputed move table based on board occupancy:
/// - Empty squares: valid moves
/// - Opponent pieces: valid captures (then stop in this direction)
/// - Own pieces: invalid moves (stop in this direction)
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the moving piece (1 for White, -1 for Black)
/// * `table` - Precomputed move table for this square and piece type
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// // Generate rook moves from square 0
/// let mut moves = Vec::new();
/// generate_sliding_moves(&game, 0, COLOR_WHITE, &game.rook[0], &mut moves);
/// ```
pub fn generate_sliding_moves(
    game: &Game,
    from: i8,
    color: Color,
    table: &[KK],
    moves: &mut Vec<KK>,
) {
    // Track which directions are blocked to avoid redundant checks
    // This is an optimization: once a direction is blocked, we skip
    // remaining moves in that direction
    let mut blocked_dirs = [false; 8];

    for candidate in table {
        // Calculate direction index to track blocked directions
        // This is a heuristic to group moves by direction
        let dir_idx = ((candidate.dst - from) / ((candidate.dst - from).abs().max(1))) as usize;

        // Skip if this direction is already blocked
        if blocked_dirs.get(dir_idx % 8).copied().unwrap_or(false) {
            continue;
        }

        let dest_piece = game.board[candidate.dst as usize];

        if dest_piece == 0 {
            // Empty square: valid move, continue in this direction
            moves.push(*candidate);
        } else if !piece_belongs_to(dest_piece, color) {
            // Opponent piece: valid capture, but block this direction
            moves.push(*candidate);
            blocked_dirs[dir_idx % 8] = true;
        } else {
            // Own piece: invalid move, block this direction
            blocked_dirs[dir_idx % 8] = true;
        }
    }
}
