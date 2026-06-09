//! Perft — move-generation correctness tests via node counting.
//!
//! Perft (from *performance test*) counts all leaf nodes at a fixed depth from
//! a given position.  Because the expected counts are universally agreed upon
//! (see <https://www.chessprogramming.org/Perft_Results>), any deviation means
//! the move generator has a bug.
//!
//! ## How it works
//!
//! ```text
//! perft(pos, depth=0) = 1          ← count this leaf
//! perft(pos, depth=N) = Σ perft(child, N-1)   for every legal child
//! ```
//!
//! "Legal" means the mover's king is not in check after the move.
//!
//! ## Usage
//!
//! Call [`perft`] to get the total node count, or [`perft_divide`] to see the
//! per-move breakdown (the standard way to isolate wrong sub-trees).
//!
//! ```ignore
//! let mut game = game_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
//! assert_eq!(perft(&mut game, 4, 1), 197_281);  // starting pos, depth 4
//! ```

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::move_gen::{generate_pseudo_legal_moves, is_in_check};
use crate::search::make_unmake::{make_move, unmake_move};
use crate::types::*;

// ── Core perft ───────────────────────────────────────────────────────────────

/// Count all legal leaf nodes at exactly `depth` plies.
///
/// `color` is the side to move: `1` = White, `-1` = Black.
pub fn perft(game: &mut Game, depth: u32, color: Color) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = generate_pseudo_legal_moves(game, color);
    let mut nodes = 0u64;

    for mv in moves {
        let undo = make_move(game, mv);

        // A pseudo-legal move is legal iff it doesn't leave the mover's king in check.
        if !is_in_check(game, color) {
            nodes += perft(game, depth - 1, -color);
        }

        unmake_move(game, mv, undo);
    }

    nodes
}

// ── Divide ───────────────────────────────────────────────────────────────────

/// Run perft and print the sub-count for each first-level move.
///
/// This is the standard debugging tool: compare your output to a reference
/// engine's `divide` output to find exactly which move has the wrong count.
#[cfg(feature = "std")]
pub fn perft_divide(game: &mut Game, depth: u32, color: Color) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = generate_pseudo_legal_moves(game, color);
    let mut total = 0u64;

    for mv in moves {
        let undo = make_move(game, mv);

        if !is_in_check(game, color) {
            let count = if depth == 1 {
                1
            } else {
                perft(game, depth - 1, -color)
            };

            let uci = mv_to_uci(mv);
            println!("{}: {}", uci, count);
            total += count;
        }

        unmake_move(game, mv, undo);
    }

    println!("\nNodes searched: {}", total);
    total
}

#[cfg(feature = "std")]
fn mv_to_uci(mv: KK) -> String {
    let file_char = |sq: i8| (b'a' + (sq % 8) as u8) as char;
    let rank_char = |sq: i8| (b'1' + (sq / 8) as u8) as char;
    let promo_id = mv.nxt_dir_idx >> 4;
    let promo_ch = match promo_id {
        5 => "q",
        4 => "r",
        3 => "b",
        2 => "n",
        _ => "",
    };
    format!(
        "{}{}{}{}{}",
        file_char(mv.src),
        rank_char(mv.src),
        file_char(mv.dst),
        rank_char(mv.dst),
        promo_ch,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    fn fen_color(fen: &str) -> Color {
        if fen.split_whitespace().nth(1) == Some("w") { 1 } else { -1 }
    }

    // ── Starting position (chessprogramming.org/Perft_Results) ───────────────

    #[cfg(feature = "std")]
    #[test]
    fn start_depth1() {
        use crate::api::game::game_from_fen;
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 1, fen_color(fen)), 20);
    }

    #[cfg(feature = "std")]
    #[test]
    fn start_depth2() {
        use crate::api::game::game_from_fen;
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 2, fen_color(fen)), 400);
    }

    #[cfg(feature = "std")]
    #[test]
    fn start_depth3() {
        use crate::api::game::game_from_fen;
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 3, fen_color(fen)), 8_902);
    }

    #[cfg(feature = "std")]
    #[test]
    fn start_depth4() {
        use crate::api::game::game_from_fen;
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 4, fen_color(fen)), 197_281);
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "slow — run with `cargo test -- --ignored`"]
    fn start_depth5() {
        use crate::api::game::game_from_fen;
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 5, fen_color(fen)), 4_865_609);
    }

    // ── Kiwipete: castling, en passant, promotions ────────────────────────────
    // Source: <https://www.chessprogramming.org/Perft_Results#Position_2>

    #[cfg(feature = "std")]
    #[test]
    fn kiwipete_depth1() {
        use crate::api::game::game_from_fen;
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 1, fen_color(fen)), 48);
    }

    #[cfg(feature = "std")]
    #[test]
    fn kiwipete_depth2() {
        use crate::api::game::game_from_fen;
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 2, fen_color(fen)), 2_039);
    }

    #[cfg(feature = "std")]
    #[test]
    fn kiwipete_depth3() {
        use crate::api::game::game_from_fen;
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 3, fen_color(fen)), 97_862);
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "slow — run with `cargo test -- --ignored`"]
    fn kiwipete_depth4() {
        use crate::api::game::game_from_fen;
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 4, fen_color(fen)), 4_085_603);
    }

    // ── Position 3: en passant edge case ─────────────────────────────────────

    #[cfg(feature = "std")]
    #[test]
    fn pos3_depth1() {
        use crate::api::game::game_from_fen;
        let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 1, fen_color(fen)), 14);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pos3_depth2() {
        use crate::api::game::game_from_fen;
        let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 2, fen_color(fen)), 191);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pos3_depth3() {
        use crate::api::game::game_from_fen;
        let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 3, fen_color(fen)), 2_812);
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "slow — run with `cargo test -- --ignored`"]
    fn pos3_depth4() {
        use crate::api::game::game_from_fen;
        let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 4, fen_color(fen)), 43_238);
    }

    // ── Position 5: promotion-heavy ───────────────────────────────────────────

    #[cfg(feature = "std")]
    #[test]
    fn pos5_depth1() {
        use crate::api::game::game_from_fen;
        let fen = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 1, fen_color(fen)), 44);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pos5_depth2() {
        use crate::api::game::game_from_fen;
        let fen = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 2, fen_color(fen)), 1_486);
    }

    #[cfg(feature = "std")]
    #[test]
    fn pos5_depth3() {
        use crate::api::game::game_from_fen;
        let fen = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
        let mut g = game_from_fen(fen);
        assert_eq!(perft(&mut g, 3, fen_color(fen)), 62_379);
    }
}
