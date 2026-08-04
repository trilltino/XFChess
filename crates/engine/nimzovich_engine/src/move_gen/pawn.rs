//! Pawn move generation
//!
//! Handles pawn-specific move generation including:
//! - Single and double forward pushes
//! - Diagonal captures
//! - En passant
//! - Promotion (emits 4 variants: Q/R/B/N)

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::board::*;
use crate::constants::*;
use crate::types::*;

/// Returns `true` if this pawn move reaches the promotion rank.
#[inline]
fn is_promotion(dst: i8, color: Color) -> bool {
    if color > 0 {
        dst / 8 == 7
    } else {
        dst / 8 == 0
    }
}

/// Push a move (or 4 promotion variants) onto `moves`.
fn push_move(moves: &mut Vec<KK>, mv: KK, color: Color) {
    if is_promotion(mv.dst, color) {
        // Encode promotion piece in the high nibble of nxt_dir_idx (bits 4-7).
        // Piece IDs: QUEEN=5, ROOK=4, BISHOP=3, KNIGHT=2
        for promo in [
            QUEEN_ID as u8,
            ROOK_ID as u8,
            BISHOP_ID as u8,
            KNIGHT_ID as u8,
        ] {
            moves.push(KK {
                nxt_dir_idx: mv.nxt_dir_idx | (promo << 4),
                ..mv
            });
        }
    } else {
        moves.push(mv);
    }
}

/// Generate pawn moves from a given square.
///
/// When `noisy_only` is set (quiescence search), quiet forward pushes are
/// skipped — except a single push that lands on the promotion rank, since
/// that's "noisy" too (it changes material). Captures and en passant are
/// always noisy and always included.
pub fn generate_pawn_moves(
    game: &Game,
    from: i8,
    color: Color,
    moves: &mut Vec<KK>,
    noisy_only: bool,
) {
    let candidates = if color > 0 {
        &game.white_pawn[from as usize]
    } else {
        &game.black_pawn[from as usize]
    };

    let (from_col, _from_row) = pos_to_square(from);
    let forward_dir = if color > 0 { 8 } else { -8 };

    for candidate in candidates {
        let to = candidate.dst;
        let (to_col, _to_row) = pos_to_square(to);
        let dest_piece = game.board[to as usize];

        let is_diagonal = (to_col - from_col).abs() == 1;

        if is_diagonal {
            // Normal capture
            if dest_piece != 0 && !piece_belongs_to(dest_piece, color) {
                push_move(moves, *candidate, color);
            }
            // En passant
            else if dest_piece == 0 {
                if let Some(target) = game.en_passant_target {
                    if to == target {
                        moves.push(*candidate); // EP can never be a promotion
                    }
                }
            }
        } else if dest_piece == 0 {
            // Forward moves
            if (to - from).abs() == 16 {
                // Double push can never be a promotion — always quiet.
                if !noisy_only {
                    let intermediate = (from as i32 + forward_dir) as i8;
                    if game.board[intermediate as usize] == 0 {
                        moves.push(*candidate);
                    }
                }
            } else if !noisy_only || is_promotion(to, color) {
                push_move(moves, *candidate, color);
            }
        }
    }
}
