//! Differential perft against shakmaty (a known-good move generator).
//!
//! When a perft count mismatches, this walks down the tree comparing
//! per-move subtree counts and legal-move sets, printing the exact FEN and
//! move where nimzovich diverges from the reference. Children are re-imported
//! via FEN, so a divergence that disappears under FEN round-tripping points at
//! make/unmake state corruption rather than move generation.
//!
//! Run with: cargo test -p nimzovich_engine --test differential_perft -- --nocapture

use std::collections::BTreeSet;

use nimzovich_engine::api::game::game_from_fen;
use nimzovich_engine::perft::perft;
use nimzovich_engine::{generate_pseudo_legal_moves, is_legal_move, Color};

use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, EnPassantMode, Position};

fn fen_color(fen: &str) -> Color {
    if fen.split_whitespace().nth(1) == Some("w") { 1 } else { -1 }
}

fn our_perft(fen: &str, depth: u32) -> u64 {
    let mut g = game_from_fen(fen);
    perft(&mut g, depth, fen_color(fen))
}

fn ref_pos(fen: &str) -> Chess {
    let setup: Fen = fen.parse().expect("bad FEN");
    setup
        .into_position(CastlingMode::Standard)
        .expect("illegal position")
}

fn ref_perft(fen: &str, depth: u32) -> u64 {
    shakmaty::perft(&ref_pos(fen), depth)
}

/// Our engine's legal moves as UCI strings (pseudo-legal + is_legal_move filter).
fn our_legal_uci(fen: &str) -> BTreeSet<String> {
    let mut g = game_from_fen(fen);
    let color = fen_color(fen);
    let moves = generate_pseudo_legal_moves(&g, color);
    let mut out = BTreeSet::new();
    for mv in moves {
        if is_legal_move(&mut g, mv.src, mv.dst, color) {
            out.insert(mv_to_uci(mv.src, mv.dst, (mv.nxt_dir_idx >> 4) as i8));
        }
    }
    out
}

fn mv_to_uci(src: i8, dst: i8, promo_id: i8) -> String {
    let file = |sq: i8| (b'a' + (sq % 8) as u8) as char;
    let rank = |sq: i8| (b'1' + (sq / 8) as u8) as char;
    let promo = match promo_id {
        5 => "q",
        4 => "r",
        3 => "b",
        2 => "n",
        _ => "",
    };
    format!("{}{}{}{}{}", file(src), rank(src), file(dst), rank(dst), promo)
}

/// Reference legal moves as (uci, child_fen) pairs.
fn ref_moves(fen: &str) -> Vec<(String, String)> {
    let pos = ref_pos(fen);
    pos.legal_moves()
        .iter()
        .map(|m| {
            let uci = UciMove::from_move(m, CastlingMode::Standard).to_string();
            let mut child = pos.clone();
            child.play_unchecked(m);
            // EnPassantMode::Always mirrors nimzovich, which records an EP target
            // after every double push regardless of capturability.
            let child_fen = Fen::from_position(child, EnPassantMode::Always).to_string();
            (uci, child_fen)
        })
        .collect()
}

/// Recursively locate and print the first divergence. Returns true if one was found.
fn drill(fen: &str, depth: u32, path: &str) -> bool {
    let ours = our_perft(fen, depth);
    let theirs = ref_perft(fen, depth);
    if ours == theirs {
        return false;
    }

    println!("\nDIVERGENCE at depth {depth} (path: {})", if path.is_empty() { "<root>" } else { path });
    println!("  fen:   {fen}");
    println!("  ours:  {ours}");
    println!("  ref:   {theirs}  (delta {:+})", ours as i64 - theirs as i64);

    // Compare legal move sets at this node.
    let ref_mvs = ref_moves(fen);
    let ref_set: BTreeSet<String> = ref_mvs.iter().map(|(u, _)| u.clone()).collect();
    let our_set = our_legal_uci(fen);

    let extra: Vec<_> = our_set.difference(&ref_set).cloned().collect();
    let missing: Vec<_> = ref_set.difference(&our_set).cloned().collect();
    if !extra.is_empty() {
        println!("  ILLEGAL moves we generate: {extra:?}");
    }
    if !missing.is_empty() {
        println!("  LEGAL moves we miss:       {missing:?}");
    }
    if !extra.is_empty() || !missing.is_empty() {
        return true; // root cause found at this node
    }

    // Move sets agree — descend into the first child whose subtree count differs.
    if depth > 1 {
        for (uci, child_fen) in &ref_mvs {
            let our_child = our_perft(child_fen, depth - 1);
            let ref_child = ref_perft(child_fen, depth - 1);
            if our_child != ref_child {
                let new_path = if path.is_empty() { uci.clone() } else { format!("{path} {uci}") };
                return drill(child_fen, depth - 1, &new_path);
            }
        }
        // Children all match after FEN round-trip, but this node's count differs:
        // the bug is state carried through make/unmake, not move generation.
        println!("  All children match via FEN re-import → make/unmake state corruption at this node.");
        return true;
    }

    true
}

#[test]
#[ignore = "diagnostic — run with --ignored --nocapture to locate movegen divergences"]
fn locate_divergences() {
    let cases: &[(&str, u32)] = &[
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 5),
        ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 4),
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 4),
        // Open Sicilian after 1.e4 c5 2.Nf3 Nc6 3.d4 cxd4 4.Nxd4 g6 — engine
        // generated illegal e7d8 here in UCI match play (knight on d4).
        ("r1bqkbnr/pp1ppp1p/2n3p1/8/3NP3/8/PPP2PPP/RNBQKB1R w KQkq - 0 5", 2),
    ];

    let mut found = false;
    for (fen, depth) in cases {
        println!("\n=== Checking {fen} at depth {depth} ===");
        if drill(fen, *depth, "") {
            found = true;
        } else {
            println!("  OK — matches reference ({} nodes)", ref_perft(fen, *depth));
        }
    }
    assert!(!found, "divergences found — see output above");
}
