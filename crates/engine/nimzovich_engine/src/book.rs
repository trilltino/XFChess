//! Built-in opening book.
//!
//! A compact set of mainstream theory lines (UCI moves from the start
//! position). [`book_move`] returns a book continuation for the game so far,
//! picking pseudo-randomly between candidate lines per process so the engine
//! doesn't play the same opening every game.
//!
//! Callers MUST validate the returned move for legality before playing it
//! (`is_legal_move`) — a typo in a line must degrade to "out of book", never
//! to an illegal move.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// Mainstream opening lines as space-separated UCI moves from startpos.
const BOOK_LINES: &[&str] = &[
    // Ruy Lopez, closed
    "e2e4 e7e5 g1f3 b8c6 f1b5 a7a6 b5a4 g8f6 e1g1 f8e7 f1e1 b7b5 a4b3 d7d6",
    // Italian, giuoco pianissimo
    "e2e4 e7e5 g1f3 b8c6 f1c4 f8c5 c2c3 g8f6 d2d3 d7d6 e1g1 e8g8",
    // Sicilian Najdorf
    "e2e4 c7c5 g1f3 d7d6 d2d4 c5d4 f3d4 g8f6 b1c3 a7a6 f1e2 e7e5 d4b3 f8e7",
    // Sicilian accelerated fianchetto
    "e2e4 c7c5 g1f3 b8c6 d2d4 c5d4 f3d4 g7g6 b1c3 f8g7 c1e3 g8f6",
    // French classical
    "e2e4 e7e6 d2d4 d7d5 b1c3 g8f6 c1g5 f8e7 e4e5 f6d7 g5e7 d8e7",
    // Caro-Kann classical
    "e2e4 c7c6 d2d4 d7d5 b1c3 d5e4 c3e4 c8f5 e4g3 f5g6 h2h4 h7h6 g1f3 b8d7",
    // Queen's Gambit Declined
    "d2d4 d7d5 c2c4 e7e6 b1c3 g8f6 c1g5 f8e7 e2e3 e8g8 g1f3 h7h6 g5h4 b7b6",
    // Slav, main line
    "d2d4 d7d5 c2c4 c7c6 g1f3 g8f6 b1c3 d5c4 a2a4 c8f5 e2e3 e7e6 f1c4 f8b4",
    // King's Indian, classical
    "d2d4 g8f6 c2c4 g7g6 b1c3 f8g7 e2e4 d7d6 g1f3 e8g8 f1e2 e7e5 e1g1 b8c6",
    // Nimzo-Indian, Rubinstein
    "d2d4 g8f6 c2c4 e7e6 b1c3 f8b4 e2e3 e8g8 f1d3 d7d5 g1f3 c7c5 e1g1 b8c6",
    // Queen's Indian
    "d2d4 g8f6 c2c4 e7e6 g1f3 b7b6 g2g3 c8a6 b2b3 f8b4 c1d2 b4e7",
    // English, reversed Sicilian
    "c2c4 e7e5 b1c3 g8f6 g1f3 b8c6 g2g3 d7d5 c4d5 f6d5 f1g2 d5b6 e1g1 f8e7",
    // English, symmetric
    "c2c4 c7c5 g1f3 g8f6 b1c3 b8c6 g2g3 d7d5 c4d5 f6d5 f1g2 d5c7",
    // Catalan, open
    "d2d4 g8f6 c2c4 e7e6 g2g3 d7d5 f1g2 f8e7 g1f3 e8g8 e1g1 d5c4 d1c2 a7a6",
    // London System
    "d2d4 d7d5 c1f4 g8f6 e2e3 c7c5 c2c3 b8c6 b1d2 e7e6 g1f3 f8d6",
    // Scotch
    "e2e4 e7e5 g1f3 b8c6 d2d4 e5d4 f3d4 g8f6 d4c6 b7c6 e4e5 d8e7 d1e2 f6d5",
    // Petroff
    "e2e4 e7e5 g1f3 g8f6 f3e5 d7d6 e5f3 f6e4 d2d4 d6d5 f1d3 b8c6",
    // Pirc, classical
    "e2e4 d7d6 d2d4 g8f6 b1c3 g7g6 g1f3 f8g7 f1e2 e8g8 e1g1 c7c6",
    // Scandinavian, main line
    "e2e4 d7d5 e4d5 d8d5 b1c3 d5a5 d2d4 g8f6 g1f3 c7c6 f1c4 c8f5",
    // Réti
    "g1f3 d7d5 g2g3 g8f6 f1g2 e7e6 e1g1 f8e7 d2d3 e8g8 b1d2 c7c5",
];

/// Returns a book continuation for a game that began at the start position
/// and has played `moves_played` (UCI strings), or `None` when out of book.
///
/// Candidate selection is hashed per process, so repeated games vary their
/// openings without an RNG dependency.
pub fn book_move(moves_played: &[&str]) -> Option<&'static str> {
    // Keep book usage to the opening proper.
    if moves_played.len() >= 16 {
        return None;
    }

    let mut candidates: Vec<&'static str> = Vec::new();
    for line in BOOK_LINES {
        let line_moves: Vec<&str> = line.split_whitespace().collect();
        if line_moves.len() > moves_played.len()
            && line_moves[..moves_played.len()] == *moves_played
        {
            let next = line_moves[moves_played.len()];
            if !candidates.contains(&next) {
                candidates.push(next);
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Process-seeded pick: RandomState differs per process, giving variety
    // across games while staying deterministic within one selection.
    use std::sync::OnceLock;
    static SEED: OnceLock<RandomState> = OnceLock::new();
    let mut hasher = SEED.get_or_init(RandomState::new).build_hasher();
    hasher.write_usize(moves_played.len());
    for m in moves_played {
        hasher.write(m.as_bytes());
    }
    let idx = (hasher.finish() as usize) % candidates.len();
    Some(candidates[idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::game::new_game;
    use crate::api::{do_move_with_promo, is_legal_move};

    fn parse(uci: &str) -> (i8, i8, i8) {
        let b = uci.as_bytes();
        let sq = |f: u8, r: u8| ((r - b'1') * 8 + (f - b'a')) as i8;
        let promo = if b.len() >= 5 {
            match b[4] {
                b'n' => 2,
                b'b' => 3,
                b'r' => 4,
                b'q' => 5,
                _ => 0,
            }
        } else {
            0
        };
        (sq(b[0], b[1]), sq(b[2], b[3]), promo)
    }

    /// Every move of every book line must be legal when replayed from the
    /// start position — a typo here must be caught by CI, not by a forfeit.
    #[test]
    fn all_book_lines_are_legal() {
        for line in BOOK_LINES {
            let mut game = new_game();
            let mut color = 1i64;
            for (i, uci) in line.split_whitespace().enumerate() {
                let (src, dst, promo) = parse(uci);
                assert!(
                    is_legal_move(&mut game, src, dst, color),
                    "illegal book move '{uci}' (ply {i}) in line: {line}"
                );
                do_move_with_promo(&mut game, src, dst, true, promo);
                color = -color;
            }
        }
    }

    #[test]
    fn book_responds_to_e4() {
        let mv = book_move(&["e2e4"]).expect("book must answer 1.e4");
        assert!(["e7e5", "c7c5", "e7e6", "c7c6", "d7d6", "d7d5"].contains(&mv));
    }

    #[test]
    fn out_of_book_returns_none() {
        assert!(book_move(&["a2a3"]).is_none());
    }
}
