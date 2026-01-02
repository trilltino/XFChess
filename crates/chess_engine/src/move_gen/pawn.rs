//! Pawn move generation
//!
//! Handles pawn-specific move generation including:
//! - Single and double forward pushes
//! - Diagonal captures
//! - En passant (future enhancement)
//! - Promotion (handled during move execution)
//!
//! ## Pawn Movement Rules
//!
//! - **Forward push**: Pawns move one square forward (toward opponent)
//! - **Double push**: From starting rank (rank 2 for white, rank 7 for black),
//!   pawns can move two squares forward
//! - **Captures**: Pawns capture diagonally forward (one square)
//! - **En passant**: If opponent pawn double-pushed past, can capture "en passant"
//! - **Promotion**: On reaching the 8th rank, pawns promote to queen/rook/bishop/knight

use crate::board::*;
use crate::types::*;

/// Generate pawn moves from a given square
///
/// This function filters the precomputed pawn move table based on:
/// - Forward moves: destination must be empty (and intermediate square for double push)
/// - Diagonal moves: destination must contain an opponent piece (capture)
///
/// # Arguments
///
/// * `game` - The current game state
/// * `from` - Source square index (0-63)
/// * `color` - Color of the pawn (1 for White, -1 for Black)
/// * `moves` - Output vector to append valid moves to
///
/// # Examples
///
/// ```rust,ignore
/// let mut moves = Vec::new();
/// generate_pawn_moves(&game, 12, COLOR_WHITE, &mut moves);
/// // Moves now contains e2-e3, e2-e4, and any diagonal captures
/// ```
pub fn generate_pawn_moves(game: &Game, from: i8, color: Color, moves: &mut Vec<KK>) {
    // Get precomputed pawn moves for this square and color
    let candidates = if color > 0 {
        &game.white_pawn[from as usize]
    } else {
        &game.black_pawn[from as usize]
    };

    let (from_col, _from_row) = pos_to_square(from);
    let forward_dir = if color > 0 { -8 } else { 8 };

    for candidate in candidates {
        let to = candidate.dst;
        let (to_col, _to_row) = pos_to_square(to);
        let dest_piece = game.board[to as usize];

        // Determine if this is a diagonal move (capture) or forward move (push)
        let is_diagonal = (to_col - from_col).abs() == 1;

        if is_diagonal {
            // Diagonal moves are only valid for captures
            if dest_piece != 0 && !piece_belongs_to(dest_piece, color) {
                moves.push(*candidate);
            }
            // Note: En passant would be handled here in a full implementation
        } else {
            // Forward moves must be to empty square
            if dest_piece == 0 {
                // For double push, check intermediate square is also empty
                if (to - from).abs() == 16 {
                    let intermediate = (from as i32 + forward_dir) as i8;
                    if game.board[intermediate as usize] == 0 {
                        moves.push(*candidate);
                    }
                } else {
                    // Single push: destination is empty, so valid
                    moves.push(*candidate);
                }
            }
        }
    }
}
