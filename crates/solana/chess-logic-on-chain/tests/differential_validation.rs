//! Differential test: the on-chain validator must agree with the engine.
//!
//! `chess-logic-on-chain::validation::is_move_legal` is the gate that decides
//! whether a move recorded on-chain (for a *staked* game) is legal. If it ever
//! diverges from `nimzovich_engine`'s legality, that is a direct exploit:
//!   - a **false accept** lets a player commit an illegal move and win money;
//!   - a **false reject** stalls a legitimate game.
//!
//! For a corpus of positions we compute the engine's exact legal move set, then
//! assert the on-chain validator accepts every legal move (no false rejects) and
//! rejects every other from→to of the side to move (no false accepts).

use chess_logic_on_chain::validation::is_move_legal;
use nimzovich_engine::api::game::game_from_fen;
use nimzovich_engine::{generate_pseudo_legal_moves, is_legal_move, Color};

fn fen_color(fen: &str) -> Color {
    if fen.split_whitespace().nth(1) == Some("w") { 1 } else { -1 }
}

fn file_ch(sq: i8) -> char { (b'a' + (sq % 8) as u8) as char }
fn rank_ch(sq: i8) -> char { (b'1' + (sq / 8) as u8) as char }

fn coord_uci(src: i8, dst: i8, promo: &str) -> String {
    format!("{}{}{}{}{}", file_ch(src), rank_ch(src), file_ch(dst), rank_ch(dst), promo)
}

type UciSet = std::collections::BTreeSet<String>;
type FromToSet = std::collections::BTreeSet<(i8, i8)>;

/// The engine's legal moves: the full-UCI set (promotions carry the piece) and
/// the set of legal (from, to) transitions ignoring the promotion piece.
fn engine_legal(fen: &str) -> (UciSet, FromToSet) {
    let mut g = game_from_fen(fen);
    let color = fen_color(fen);
    let mut uci = UciSet::new();
    let mut from_to = FromToSet::new();
    for mv in generate_pseudo_legal_moves(&g, color) {
        if is_legal_move(&mut g, mv.src, mv.dst, color) {
            // promo id lives in the high nibble of nxt_dir_idx (5=q,4=r,3=b,2=n)
            let promo = match (mv.nxt_dir_idx >> 4) as i8 {
                5 => "q",
                4 => "r",
                3 => "b",
                2 => "n",
                _ => "",
            };
            uci.insert(coord_uci(mv.src, mv.dst, promo));
            from_to.insert((mv.src, mv.dst));
        }
    }
    (uci, from_to)
}

/// Squares occupied by the side to move (engine board is index 0..64).
fn from_squares(fen: &str) -> Vec<i8> {
    let g = game_from_fen(fen);
    let color = fen_color(fen);
    (0..64i8)
        .filter(|&sq| {
            let p = g.board[sq as usize];
            p != 0 && (p > 0) == (color > 0)
        })
        .collect()
}

const CORPUS: &[&str] = &[
    // Start position.
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    // After 1.e4 — black to move.
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    // Kiwipete — castling, pins, checks.
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    // Promotion-rich position (CPW position 4).
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    // En-passant available.
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
    // King in check — only check-evasions are legal.
    "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
];

#[test]
fn on_chain_validator_matches_engine() {
    let promos = ["", "q", "r", "b", "n"];
    let mut false_rejects = Vec::new();
    let mut false_accepts = Vec::new();

    for fen in CORPUS {
        let (legal_uci, legal_from_to) = engine_legal(fen);

        // No false rejects: every engine-legal move must be accepted on-chain.
        for uci in &legal_uci {
            if !is_move_legal(fen, uci) {
                false_rejects.push(format!("{fen}  rejected legal {uci}"));
            }
        }

        // No false accepts: every (from, to) transition the engine considers
        // ILLEGAL must be rejected on-chain — under any promotion suffix. We key
        // on (from, to) rather than exact UCI so a benign redundant promo suffix
        // on an otherwise-legal move (e.g. "b1a3q") is not counted: it yields the
        // same legal board transition. A genuinely illegal transition accepted
        // here would be an on-chain exploit.
        for &src in &from_squares(fen) {
            for dst in 0..64i8 {
                if src == dst || legal_from_to.contains(&(src, dst)) {
                    continue;
                }
                for promo in promos {
                    let uci = coord_uci(src, dst, promo);
                    if is_move_legal(fen, &uci) {
                        false_accepts.push(format!("{fen}  accepted ILLEGAL {uci}"));
                    }
                }
            }
        }
    }

    assert!(
        false_rejects.is_empty() && false_accepts.is_empty(),
        "on-chain validator diverges from engine:\n  FALSE ACCEPTS (exploitable):\n    {}\n  FALSE REJECTS:\n    {}",
        false_accepts.join("\n    "),
        false_rejects.join("\n    "),
    );
}
