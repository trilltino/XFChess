//! King move generation
//!
//! Handles king-specific move generation including standard moves and castling.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::board::*;
use crate::constants::*;
use crate::move_gen::attack::is_square_attacked;
use crate::types::*;

/// Generate king moves from a given square
pub fn generate_king_moves(
    game: &Game,
    from: i8,
    color: Color,
    moves: &mut Vec<KK>,
    noisy_only: bool,
) {
    // Standard king moves
    for candidate in &game.king[from as usize] {
        let dest_piece = game.board[candidate.dst as usize];
        if noisy_only {
            if dest_piece != 0 && !piece_belongs_to(dest_piece, color) {
                moves.push(*candidate);
            }
        } else if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
            moves.push(*candidate);
        }
    }

    // Castling is never a capture — skip entirely for quiescence.
    if noisy_only {
        return;
    }

    // Castling logic
    if color > 0 {
        // White castling
        if from == WK4 as i8 && !game.white_king_has_moved {
            // Kingside (e1 -> g1)
            if !game.white_rook_7_has_moved
                && game.board[5] == 0
                && game.board[6] == 0
                && !is_square_attacked(game, 4, COLOR_BLACK)
                && !is_square_attacked(game, 5, COLOR_BLACK)
                && !is_square_attacked(game, 6, COLOR_BLACK)
            {
                moves.push(KK::new(4, 6, 0, 0));
            }
            // Queenside (e1 -> c1)
            if !game.white_rook_0_has_moved
                && game.board[1] == 0
                && game.board[2] == 0
                && game.board[3] == 0
                && !is_square_attacked(game, 4, COLOR_BLACK)
                && !is_square_attacked(game, 3, COLOR_BLACK)
                && !is_square_attacked(game, 2, COLOR_BLACK)
            {
                moves.push(KK::new(4, 2, 0, 0));
            }
        }
    } else {
        // Black castling
        if from == BK60 as i8 && !game.black_king_has_moved {
            // Kingside (e8 -> g8)
            if !game.black_rook_63_has_moved
                && game.board[61] == 0
                && game.board[62] == 0
                && !is_square_attacked(game, 60, COLOR_WHITE)
                && !is_square_attacked(game, 61, COLOR_WHITE)
                && !is_square_attacked(game, 62, COLOR_WHITE)
            {
                moves.push(KK::new(60, 62, 0, 0));
            }
            // Queenside (e8 -> c8)
            if !game.black_rook_56_has_moved
                && game.board[57] == 0
                && game.board[58] == 0
                && game.board[59] == 0
                && !is_square_attacked(game, 60, COLOR_WHITE)
                && !is_square_attacked(game, 59, COLOR_WHITE)
                && !is_square_attacked(game, 58, COLOR_WHITE)
            {
                moves.push(KK::new(60, 58, 0, 0));
            }
        }
    }
}
