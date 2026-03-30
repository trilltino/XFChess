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

/// Map a direction delta to an index 0-7
///
/// Directions are mapped as follows:
/// - 0: North (-8)
/// - 1: South (+8)
/// - 2: East (+1)
/// - 3: West (-1)
/// - 4: NE (-7)
/// - 5: SE (+9)
/// - 6: SW (+7)
/// - 7: NW (-9)
#[inline]
fn direction_to_index(from: i8, to: i8) -> usize {
    let (from_col, from_row) = pos_to_square(from);
    let (to_col, to_row) = pos_to_square(to);

    let col_delta = (to_col - from_col).signum();
    let row_delta = (to_row - from_row).signum();

    // Map (col_delta, row_delta) to index
    match (col_delta, row_delta) {
        (0, -1) => 0,  // North
        (0, 1) => 1,   // South
        (1, 0) => 2,   // East
        (-1, 0) => 3,  // West
        (1, -1) => 4,  // NE
        (1, 1) => 5,   // SE
        (-1, 1) => 6,  // SW
        (-1, -1) => 7, // NW
        _ => 0,        // Fallback (shouldn't happen for valid sliding moves)
    }
}

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
        // Calculate direction index using column/row deltas
        let dir_idx = direction_to_index(from, candidate.dst);

        // Skip if this direction is already blocked
        if blocked_dirs[dir_idx] {
            continue;
        }

        let dest_piece = game.board[candidate.dst as usize];

        if dest_piece == 0 {
            // Empty square: valid move, continue in this direction
            moves.push(*candidate);
        } else if !piece_belongs_to(dest_piece, color) {
            // Opponent piece: valid capture, but block this direction
            moves.push(*candidate);
            blocked_dirs[dir_idx] = true;
        } else {
            // Own piece: invalid move, block this direction
            blocked_dirs[dir_idx] = true;
        }
    }
}
