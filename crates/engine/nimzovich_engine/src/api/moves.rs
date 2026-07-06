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
pub fn do_move_with_promo(
    game: &mut Game,
    src: i8,
    dst: i8,
    update_flags: bool,
    promo: i8,
) -> bool {
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
                if dst == 6 {
                    game.board[7] = 0;
                    game.board[5] = W_ROOK;
                } else if dst == 2 {
                    game.board[0] = 0;
                    game.board[3] = W_ROOK;
                }
            } else {
                if dst == 62 {
                    game.board[63] = 0;
                    game.board[61] = B_ROOK;
                } else if dst == 58 {
                    game.board[56] = 0;
                    game.board[59] = B_ROOK;
                }
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
            0 => game.white_rook_0_has_moved = true,
            4 => game.white_king_has_moved = true,
            7 => game.white_rook_7_has_moved = true,
            56 => game.black_rook_56_has_moved = true,
            60 => game.black_king_has_moved = true,
            63 => game.black_rook_63_has_moved = true,
            _ => {}
        }
        match dst {
            0 => game.white_rook_0_has_moved = true,
            7 => game.white_rook_7_has_moved = true,
            56 => game.black_rook_56_has_moved = true,
            63 => game.black_rook_63_has_moved = true,
            _ => {}
        }
    }

    // 5. Handle Promotion
    let final_piece = if piece_type == PAWN_ID && (dst / 8 == 0 || dst / 8 == 7) {
        // Use caller-specified piece, defaulting to queen
        let promo_type = if promo != 0 { promo.abs() } else { QUEEN_ID };
        if color > 0 {
            promo_type
        } else {
            -promo_type
        }
    } else {
        piece
    };

    // 6. Update halfmove clock
    let is_capture = game.board[dst as usize] != 0
        || (piece_type == PAWN_ID && Some(dst) == game.en_passant_target);
    if piece_type == PAWN_ID || is_capture {
        game.halfmove_clock = 0;
    } else {
        game.halfmove_clock += 1;
    }

    // 7. Execute move
    game.board[dst as usize] = final_piece;
    game.board[src as usize] = 0;
    game.move_counter += 1;

    // 8. Sync bitboards — generate_pseudo_legal_moves reads bitboards,
    //    not the mailbox, so they must stay consistent after every move.
    crate::board::init_bitboards(game);

    // 9. Sync the incremental Zobrist hash. The search's TT keys on
    //    game.current_hash; if API-level moves don't refresh it, every search
    //    after the first probes the TT with a stale root hash and returns the
    //    previous position's best move (observed as illegal-move forfeits in
    //    UCI matches). move_counter was already incremented above, so the
    //    side-to-move term is correct.
    #[cfg(feature = "search")]
    {
        game.current_hash = crate::hash::compute_full_hash(game);
        // Record the new position for repetition detection (the search scans
        // this history, bounded by the halfmove clock).
        game.hash_history.push(game.current_hash);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::game::new_game;
    use crate::constants::COLOR_WHITE;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn bitboards_match_mailbox(game: &Game) {
        for sq in 0usize..64 {
            let piece = game.board[sq];
            let bit: u64 = 1 << sq;
            assert_eq!(
                (game.white_pawns.0 & bit) != 0,
                piece == 1,
                "white_pawns sq {sq}"
            );
            assert_eq!(
                (game.white_knights.0 & bit) != 0,
                piece == 2,
                "white_knights sq {sq}"
            );
            assert_eq!(
                (game.white_bishops.0 & bit) != 0,
                piece == 3,
                "white_bishops sq {sq}"
            );
            assert_eq!(
                (game.white_rooks.0 & bit) != 0,
                piece == 4,
                "white_rooks sq {sq}"
            );
            assert_eq!(
                (game.white_queens.0 & bit) != 0,
                piece == 5,
                "white_queens sq {sq}"
            );
            assert_eq!(
                (game.white_kings.0 & bit) != 0,
                piece == 6,
                "white_kings sq {sq}"
            );
            assert_eq!(
                (game.black_pawns.0 & bit) != 0,
                piece == -1,
                "black_pawns sq {sq}"
            );
            assert_eq!(
                (game.black_knights.0 & bit) != 0,
                piece == -2,
                "black_knights sq {sq}"
            );
            assert_eq!(
                (game.black_bishops.0 & bit) != 0,
                piece == -3,
                "black_bishops sq {sq}"
            );
            assert_eq!(
                (game.black_rooks.0 & bit) != 0,
                piece == -4,
                "black_rooks sq {sq}"
            );
            assert_eq!(
                (game.black_queens.0 & bit) != 0,
                piece == -5,
                "black_queens sq {sq}"
            );
            assert_eq!(
                (game.black_kings.0 & bit) != 0,
                piece == -6,
                "black_kings sq {sq}"
            );
        }
    }

    // ── bitboard sync tests ───────────────────────────────────────────────────

    #[test]
    fn bitboards_in_sync_after_simple_move() {
        let mut game = new_game();
        // e2–e4: white pawn from sq 12 to sq 28
        do_move(&mut game, 12, 28, true);
        bitboards_match_mailbox(&game);
        assert!(
            (game.white_pawns.0 & (1 << 28)) != 0,
            "pawn should be on e4"
        );
        assert!((game.white_pawns.0 & (1 << 12)) == 0, "e2 should be empty");
    }

    #[test]
    fn bitboards_in_sync_after_white_kingside_castle() {
        // Position with only kings and rooks, ready to castle
        let mut game = new_game();
        // Clear f1, g1 to allow O-O
        game.board[5] = 0;
        game.board[6] = 0;
        crate::board::init_bitboards(&mut game);
        // King e1(4) → g1(6)
        do_move(&mut game, 4, 6, true);
        bitboards_match_mailbox(&game);
        assert!((game.white_kings.0 & (1 << 6)) != 0, "king on g1");
        assert!((game.white_rooks.0 & (1 << 5)) != 0, "rook on f1");
        assert!((game.white_rooks.0 & (1 << 7)) == 0, "h1 empty");
        assert_eq!(game.board[6], 6, "king piece on g1");
        assert_eq!(game.board[5], 4, "rook piece on f1");
        assert_eq!(game.board[7], 0, "h1 empty mailbox");
    }

    #[test]
    fn bitboards_in_sync_after_black_kingside_castle() {
        let mut game = new_game();
        game.board[61] = 0;
        game.board[62] = 0;
        crate::board::init_bitboards(&mut game);
        game.move_counter = 1; // black's turn
                               // King e8(60) → g8(62)
        do_move(&mut game, 60, 62, true);
        bitboards_match_mailbox(&game);
        assert!((game.black_kings.0 & (1 << 62)) != 0, "black king on g8");
        assert!((game.black_rooks.0 & (1 << 61)) != 0, "black rook on f8");
        assert!((game.black_rooks.0 & (1 << 63)) == 0, "h8 empty");
    }

    #[test]
    fn bitboards_in_sync_after_capture() {
        let mut game = new_game();
        // Put a black pawn on e4 manually
        game.board[28] = -1;
        crate::board::init_bitboards(&mut game);
        // White pawn d2(11) → d4(27) first
        do_move(&mut game, 11, 27, true);
        bitboards_match_mailbox(&game);
        // Now white pawn d4(27) captures e4 pawn? No — let's use a cleaner capture:
        // Put white knight on f3(21), black pawn on e5(36)
        let mut game2 = new_game();
        game2.board[21] = 2; // white knight on f3
        game2.board[36] = -1; // black pawn on e5
        crate::board::init_bitboards(&mut game2);
        do_move(&mut game2, 21, 36, false); // Nxe5
        bitboards_match_mailbox(&game2);
        assert!((game2.white_knights.0 & (1 << 36)) != 0);
        assert!((game2.black_pawns.0 & (1 << 36)) == 0);
    }

    #[test]
    fn bitboards_in_sync_after_promotion() {
        let mut game = new_game();
        // White pawn on a7(48), clear a8(56)
        game.board[48] = 1;
        game.board[56] = 0;
        crate::board::init_bitboards(&mut game);
        do_move(&mut game, 48, 56, false); // a7-a8=Q
        bitboards_match_mailbox(&game);
        assert!(
            (game.white_queens.0 & (1 << 56)) != 0,
            "promoted queen on a8"
        );
        assert!((game.white_pawns.0 & (1 << 56)) == 0);
    }

    // ── is_legal_move restore correctness ────────────────────────────────────

    #[test]
    fn is_legal_move_restores_bitboards() {
        let mut game = new_game();
        // Check a legal move and then verify state is fully restored
        let snap_board = game.board;
        let snap_wk = game.white_kings.0;
        let snap_wp = game.white_pawns.0;
        is_legal_move(&mut game, 12, 28, COLOR_WHITE); // e2-e4 legal check
        assert_eq!(game.board, snap_board, "mailbox should be restored");
        assert_eq!(game.white_kings.0, snap_wk, "bitboard should be restored");
        assert_eq!(game.white_pawns.0, snap_wp, "bitboard should be restored");
    }

    // ── PGN / SAN tests using real game moves ─────────────────────────────────

    #[cfg(feature = "std")]
    #[test]
    fn san_to_move_ne5_after_double_castling() {
        // Reproduces the Immortal Zugzwang bug: Ne5 was returning IllegalMove
        // because do_move didn't keep bitboards in sync after the castling moves.
        use crate::pgn::san_to_move;
        let mut game = new_game();
        let moves = [
            "d4", "Nf6", "c4", "e6", "Nf3", "b6", "g3", "Bb7", "Bg2", "Be7", "Nc3", "O-O", "O-O",
            "d5",
        ];
        for san in &moves {
            let (src, dst, _) = san_to_move(&mut game, san)
                .unwrap_or_else(|e| panic!("san_to_move '{}' failed: {:?}", san, e));
            do_move(&mut game, src, dst, true);
        }
        // Move 15: Ne5 — this previously returned IllegalMove
        let result = san_to_move(&mut game, "Ne5");
        assert!(
            result.is_ok(),
            "Ne5 must be legal after O-O O-O, got: {:?}",
            result
        );
        let (src, dst, _) = result.unwrap();
        assert_eq!(src, 21, "knight should be on f3 (sq 21)");
        assert_eq!(dst, 36, "knight should move to e5 (sq 36)");
    }

    #[cfg(feature = "std")]
    #[test]
    fn zugzwang_pgn_precomputes_all_50_plies() {
        use crate::pgn::{parse_pgn, san_to_move};
        let pgn = "
1. d4 Nf6 2. c4 e6 3. Nf3 b6 4. g3 Bb7 5. Bg2 Be7
6. Nc3 O-O 7. O-O d5 8. Ne5 c6
9. cxd5 cxd5 10. Bf4 a6
11. Rc1 b5 12. Qb3 Nc6
13. Nxc6 Bxc6 14. h3 Qd7 15. Kh2 Nh5
16. Bd2 f5 17. Qd1 b4 18. Nb1 Bb5 19. Rg1 Bd6 20. e4 fxe4
21. Qxh5 Rxf2 22. Qg5 Raf8 23. Kh1 R8f5 24. Qe3 Bd3 25. Rce1 h6
0-1";
        let parsed = parse_pgn(pgn).unwrap();
        let expected = parsed.moves.len();
        let mut game = new_game();
        let mut executed = 0usize;
        for san in &parsed.moves {
            let (src, dst, _) = san_to_move(&mut game, san)
                .unwrap_or_else(|e| panic!("ply {executed}: san_to_move '{san}' failed: {e:?}"));
            do_move(&mut game, src, dst, true);
            executed += 1;
        }
        assert_eq!(executed, expected, "all {} plies should execute", expected);
    }
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
            let halfmove_before = game.halfmove_clock;
            let move_counter_before = game.move_counter;
            #[cfg(feature = "search")]
            let hash_before = game.current_hash;

            do_move(game, src, dst, false);
            let legal = !is_in_check(game, color);

            // Restore state (do_move pushed a hash-history entry — pop it)
            game.board = board_before;
            game.en_passant_target = ep_before;
            game.halfmove_clock = halfmove_before;
            game.move_counter = move_counter_before;
            crate::board::init_bitboards(game);
            #[cfg(feature = "search")]
            {
                game.hash_history.pop();
                game.current_hash = hash_before;
            }

            return legal;
        }
    }

    false
}
