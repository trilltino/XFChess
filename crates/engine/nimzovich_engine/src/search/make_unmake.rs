//! Move making and unmaking for search
//!
//! Provides functions to make and unmake moves during search, with undo
//! information to restore the board state.

use crate::hash::{toggle_turn, update_hash};
use crate::board::update_bitboards;
use crate::constants::*;
use crate::types::*;

/// Information needed to fully undo a move during search
pub(crate) struct UndoInfo {
    pub captured_piece: i8,
    pub from_square_piece: i8,
    pub old_ep_target: Option<i8>,
    pub old_halfmove_clock: u32,
    pub flags: [bool; 6], // WK, BK, WR0, WR7, BR56, BR63
}

/// Make a move on the board (returns undo information).
/// Handles special moves: Castling, En Passant, and Promotion.
pub(crate) fn make_move(game: &mut Game, mv: KK) -> UndoInfo {
    let src = mv.src as usize;
    let dst = mv.dst as usize;
    let moving_piece = game.board[src];
    let mut captured_piece = game.board[dst];
    let piece_type = moving_piece.abs();
    let color = if moving_piece > 0 { 1 } else { -1 };

    let undo = UndoInfo {
        captured_piece,
        from_square_piece: moving_piece,
        old_ep_target: game.en_passant_target,
        old_halfmove_clock: game.halfmove_clock,
        flags: [
            game.white_king_has_moved,
            game.black_king_has_moved,
            game.white_rook_0_has_moved,
            game.white_rook_7_has_moved,
            game.black_rook_56_has_moved,
            game.black_rook_63_has_moved,
        ],
    };

    // 1. Handle En Passant Capture
    if piece_type == PAWN_ID && game.en_passant_target == Some(dst as i8) {
        let cap_sq = if color > 0 { dst - 8 } else { dst + 8 };
        captured_piece = game.board[cap_sq];
        update_hash(game, cap_sq, captured_piece);
        update_bitboards(game, cap_sq, captured_piece);
        game.board[cap_sq] = 0;
    }

    // 2. Handle Castling (Move the Rook)
    if piece_type == KING_ID && (dst as i32 - src as i32).abs() == 2 {
        let (r_src, r_dst) = if color > 0 {
            if dst == 6 { (7, 5) } else { (0, 3) }
        } else {
            if dst == 62 { (63, 61) } else { (56, 59) }
        };
        let rook = game.board[r_src];
        update_hash(game, r_src, rook);
        update_bitboards(game, r_src, rook);
        game.board[r_src] = 0;
        game.board[r_dst] = rook;
        update_hash(game, r_dst, rook);
        update_bitboards(game, r_dst, rook);
    }

    // 3. Update EP Target
    game.en_passant_target = None;
    if piece_type == PAWN_ID && (dst as i32 - src as i32).abs() == 16 {
        game.en_passant_target = Some(if color > 0 { src as i8 + 8 } else { src as i8 - 8 });
    }

    // 4. Update Castling Flags
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

    // 5. Handle Promotion
    let promo_id = (mv.nxt_dir_idx >> 4) as i8;
    let final_piece = if promo_id != 0 {
        if color > 0 { promo_id } else { -promo_id }
    } else {
        moving_piece
    };

    // 6. Execute Move (standard updates)
    update_hash(game, src, moving_piece);
    update_hash(game, dst, undo.captured_piece); // captured_piece might be 0
    update_bitboards(game, src, moving_piece);
    update_bitboards(game, dst, undo.captured_piece);

    game.board[dst] = final_piece;
    game.board[src] = 0;
    game.move_counter += 1;

    update_hash(game, dst, final_piece);
    update_bitboards(game, dst, final_piece);
    toggle_turn(game);

    // Halfmove clock: reset on pawn moves and captures (incl. en passant via
    // the reassigned `captured_piece`), otherwise increment.
    if piece_type == PAWN_ID || captured_piece != 0 {
        game.halfmove_clock = 0;
    } else {
        game.halfmove_clock += 1;
    }

    // Record the new position for repetition detection.
    game.hash_history.push(game.current_hash);

    undo
}

/// Unmake a move on the board.
pub(crate) fn unmake_move(game: &mut Game, mv: KK, undo: UndoInfo) {
    let src = mv.src as usize;
    let dst = mv.dst as usize;
    let color = if undo.from_square_piece > 0 { 1 } else { -1 };
    let piece_type = undo.from_square_piece.abs();

    game.hash_history.pop();
    game.halfmove_clock = undo.old_halfmove_clock;

    toggle_turn(game);

    // Remove the piece currently at dst (could be promoted)
    let current_dst_piece = game.board[dst];
    update_hash(game, dst, current_dst_piece);
    update_bitboards(game, dst, current_dst_piece);

    // Restore captured piece at dst
    update_hash(game, dst, undo.captured_piece);
    update_bitboards(game, dst, undo.captured_piece);
    game.board[dst] = undo.captured_piece;

    // Restore moving piece at src
    update_hash(game, src, undo.from_square_piece);
    update_bitboards(game, src, undo.from_square_piece);
    game.board[src] = undo.from_square_piece;

    // Handle En Passant Restore
    if piece_type == PAWN_ID && undo.old_ep_target == Some(dst as i8) {
        let cap_sq = if color > 0 { dst - 8 } else { dst + 8 };
        // We know what was captured: it was a pawn of opposite color
        let cap_pawn = if color > 0 { B_PAWN } else { W_PAWN };
        game.board[cap_sq] = cap_pawn;
        update_hash(game, cap_sq, cap_pawn);
        update_bitboards(game, cap_sq, cap_pawn);
    }

    // Handle Castling Restore
    if piece_type == KING_ID && (dst as i32 - src as i32).abs() == 2 {
        let (r_src, r_dst) = if color > 0 {
            if dst == 6 { (7, 5) } else { (0, 3) }
        } else {
            if dst == 62 { (63, 61) } else { (56, 59) }
        };
        let rook = if color > 0 { W_ROOK } else { B_ROOK };
        game.board[r_dst] = 0;
        update_hash(game, r_dst, rook);
        update_bitboards(game, r_dst, rook);
        game.board[r_src] = rook;
        update_hash(game, r_src, rook);
        update_bitboards(game, r_src, rook);
    }

    // Restore flags and EP
    game.en_passant_target = undo.old_ep_target;
    game.white_king_has_moved = undo.flags[0];
    game.black_king_has_moved = undo.flags[1];
    game.white_rook_0_has_moved = undo.flags[2];
    game.white_rook_7_has_moved = undo.flags[3];
    game.black_rook_56_has_moved = undo.flags[4];
    game.black_rook_63_has_moved = undo.flags[5];
    game.move_counter -= 1;
}
