//! Move execution and validation
//!
//! Functions for executing moves and checking move legality.

use crate::board::*;
use crate::constants::*;
use crate::move_gen::*;
use crate::types::*;

/// Execute a move on the board.
/// `promo` — if non-zero, use this piece ID when promoting a pawn (±2=N,±3=B,±4=R,±5=Q).
/// Pass 0 to default to queen promotion.
pub fn do_move(game: &mut Game, src: i8, dst: i8, update_flags: bool) -> bool {
    do_move_with_promo(game, src, dst, update_flags, 0)
}

/// Like `do_move` but accepts an explicit promotion piece ID.
pub fn do_move_with_promo(game: &mut Game, src: i8, dst: i8, update_flags: bool, promo: i8) -> bool {
    if src < 0 || src >= 64 || dst < 0 || dst >= 64 {
        return false;
    }

    let piece = game.board[src as usize];
    if piece == 0 {
        return false;
    }

    let piece_type = piece.abs();
    let color = get_piece_color(piece);

    // 1. Handle En Passant Capture
    if piece_type == PAWN_ID {
        if let Some(target) = game.en_passant_target {
            if dst == target {
                let captured_sq = if color > 0 { dst - 8 } else { dst + 8 };
                game.board[captured_sq as usize] = 0;
            }
        }
    }

    // 2. Handle Castling (Move the Rook)
    if piece_type == KING_ID {
        if (dst - src).abs() == 2 {
            if color > 0 {
                if dst == 6 { game.board[7] = 0; game.board[5] = W_ROOK; }
                else if dst == 2 { game.board[0] = 0; game.board[3] = W_ROOK; }
            } else {
                if dst == 62 { game.board[63] = 0; game.board[61] = B_ROOK; }
                else if dst == 58 { game.board[56] = 0; game.board[59] = B_ROOK; }
            }
        }
    }

    // 3. Update EP Target
    game.en_passant_target = None;
    if piece_type == PAWN_ID && (dst - src).abs() == 16 {
        game.en_passant_target = Some(if color > 0 { src + 8 } else { src - 8 });
    }

    // 4. Update Castling Flags
    if update_flags {
        match src {
            0  => game.white_rook_0_has_moved  = true,
            4  => game.white_king_has_moved     = true,
            7  => game.white_rook_7_has_moved   = true,
            56 => game.black_rook_56_has_moved  = true,
            60 => game.black_king_has_moved      = true,
            63 => game.black_rook_63_has_moved  = true,
            _ => {}
        }
        match dst {
            0  => game.white_rook_0_has_moved  = true,
            7  => game.white_rook_7_has_moved   = true,
            56 => game.black_rook_56_has_moved  = true,
            63 => game.black_rook_63_has_moved  = true,
            _ => {}
        }
    }

    // 5. Handle Promotion
    let final_piece = if piece_type == PAWN_ID && (dst / 8 == 0 || dst / 8 == 7) {
        // Use caller-specified piece, defaulting to queen
        let promo_type = if promo != 0 { promo.abs() } else { QUEEN_ID };
        if color > 0 { promo_type } else { -promo_type }
    } else {
        piece
    };

    // 6. Execute move
    game.board[dst as usize] = final_piece;
    game.board[src as usize] = 0;
    game.move_counter += 1;

    true
}

/// Check if a move is legal
pub fn is_legal_move(game: &mut Game, src: i8, dst: i8, color: Color) -> bool {
    if src < 0 || src >= 64 || dst < 0 || dst >= 64 {
        return false;
    }

    let piece = game.board[src as usize];
    if piece == 0 || !piece_belongs_to(piece, color) {
        return false;
    }

    let moves = generate_pseudo_legal_moves(game, color);
    for mv in moves {
        if mv.src == src && mv.dst == dst {
            // Simulate move safely
            let board_before = game.board;
            let ep_before = game.en_passant_target;
            
            do_move(game, src, dst, false);
            let legal = !is_in_check(game, color);
            
            // Restore state
            game.board = board_before;
            game.en_passant_target = ep_before;
            
            return legal;
        }
    }

    false
}
