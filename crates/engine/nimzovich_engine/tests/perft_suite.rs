//! Canonical perft suite — the gold-standard move-generation guard.
//!
//! Asserts exact node counts at fixed depths for the standard test positions
//! (startpos, Kiwipete, and CPW positions 3–6). These counts are independent
//! ground truth (https://www.chessprogramming.org/Perft_Results), so this test
//! needs no reference engine and runs in CI on every PR. A single wrong count
//! catches almost any move-generation, make/unmake, castling, en-passant, or
//! promotion bug.
//!
//! The deeper differential walk against shakmaty lives in `differential_perft.rs`.

use nimzovich_engine::api::game::game_from_fen;
use nimzovich_engine::perft::perft;
use nimzovich_engine::Color;

fn fen_color(fen: &str) -> Color {
    if fen.split_whitespace().nth(1) == Some("w") { 1 } else { -1 }
}

fn perft_count(fen: &str, depth: u32) -> u64 {
    let mut g = game_from_fen(fen);
    perft(&mut g, depth, fen_color(fen))
}

/// (FEN, [(depth, expected_nodes), …]). Depths chosen to keep the whole suite
/// to ~1M nodes total so it stays CI-fast.
const CASES: &[(&str, &[(u32, u64)])] = &[
    // Starting position.
    (
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[(1, 20), (2, 400), (3, 8902), (4, 197281)],
    ),
    // Kiwipete — dense tactical position, exercises castling/pins/checks.
    (
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[(1, 48), (2, 2039), (3, 97862)],
    ),
    // Position 3 — sparse endgame, lots of en-passant edge cases.
    (
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[(1, 14), (2, 191), (3, 2812), (4, 43238)],
    ),
    // Position 4 — promotions and pins (and its mirror is symmetric).
    (
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &[(1, 6), (2, 264), (3, 9467)],
    ),
    // Position 5 — castling/promotion bugs surface here.
    (
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        &[(1, 44), (2, 1486), (3, 62379)],
    ),
    // Position 6 — well-balanced middlegame.
    (
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        &[(1, 46), (2, 2079), (3, 89890)],
    ),
];

fn check(cases: &[(&str, &[(u32, u64)])]) {
    let mut failures = Vec::new();
    for (fen, depths) in cases {
        for (depth, expected) in *depths {
            let got = perft_count(fen, *depth);
            if got != *expected {
                failures.push(format!(
                    "perft({depth}) on {fen}\n    expected {expected}, got {got}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "perft mismatches (move-generation regression):\n{}",
        failures.join("\n")
    );
}

#[test]
fn perft_known_counts() {
    check(CASES);
}

/// Deeper counts — slower (~10s+). Run with `--ignored` locally or in nightly CI.
#[test]
#[ignore = "slow: run with --ignored or in nightly CI"]
fn perft_known_counts_deep() {
    check(&[
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", &[(5, 4865609)]),
        ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &[(4, 4085603)]),
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", &[(5, 674624)]),
    ]);
}
