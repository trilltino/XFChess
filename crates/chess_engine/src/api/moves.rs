//! Move execution and validation
//!
//! Functions for executing moves and checking move legality.

use crate::board::*;
use crate::move_gen::*;
use crate::types::*;

/// Execute a move on the board
///
/// # Arguments
///
/// * `game` - The game state
/// * `src` - Source square index (0-63)
/// * `dst` - Destination square index (0-63)
/// * `update_flags` - Whether to update castling rights
///
/// # Returns
///
/// `true` if the move was executed successfully, `false` if there was no piece at the source square.
///
/// # Errors
///
/// This function does not validate move legality. Use [`is_legal_move`] first to ensure
/// the move is valid before calling this function.
///
/// # Examples
///
/// ```rust,ignore
/// // Move pawn from e2 to e4
/// let success = do_move(&mut game, 12, 28, true);
/// assert!(success);
/// ```
pub fn do_move(game: &mut Game, src: i8, dst: i8, update_flags: bool) -> bool {
    // Validate square indices
    if src < 0 || src >= 64 || dst < 0 || dst >= 64 {
        return false;
    }

    let piece = game.board[src as usize];

    if piece == 0 {
        return false;
    }

    // Update castling rights if needed
    if update_flags {
        match src {
            0 => game.white_rook_0_has_moved = true,
            3 => game.white_king_has_moved = true,
            7 => game.white_rook_7_has_moved = true,
            56 => game.black_rook_56_has_moved = true,
            59 => game.black_king_has_moved = true,
            63 => game.black_rook_63_has_moved = true,
            _ => {}
        }
    }

    // Execute move
    game.board[dst as usize] = piece;
    game.board[src as usize] = 0;
    game.move_counter += 1;

    true
}

/// Check if a move is legal
///
/// Validates that:
/// - Source square contains a piece
/// - Piece belongs to the specified color
/// - Move is pseudo-legal (piece can move to destination)
/// - Move doesn't leave the king in check
///
/// # Arguments
///
/// * `game` - The game state
/// * `src` - Source square index (0-63)
/// * `dst` - Destination square index (0-63)
/// * `color` - Color of the player making the move (1 for White, -1 for Black)
///
/// # Returns
///
/// `true` if the move is legal, `false` otherwise.
///
/// # Examples
///
/// ```rust,ignore
/// // Check if e2-e4 is legal for White
/// let legal = is_legal_move(&mut game, 12, 28, 1);
/// ```
pub fn is_legal_move(game: &mut Game, src: i8, dst: i8, color: Color) -> bool {
    // Validate square indices
    if src < 0 || src >= 64 || dst < 0 || dst >= 64 {
        return false;
    }

    let piece = game.board[src as usize];

    if piece == 0 {
        return false;
    }

    if !piece_belongs_to(piece, color) {
        return false;
    }

    // Generate legal moves for this piece
    let moves = generate_pseudo_legal_moves(game, color);

    for mv in moves {
        if mv.src == src && mv.dst == dst {
            // Verify it doesn't leave king in check
            let captured = game.board[dst as usize];
            game.board[dst as usize] = piece;
            game.board[src as usize] = 0;

            let legal = !is_in_check(game, color);

            game.board[src as usize] = piece;
            game.board[dst as usize] = captured;

            return legal;
        }
    }

    false
}
