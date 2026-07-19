//! Alpha-beta search with iterative deepening
//!
//! This module implements the core AI search using:
//! - Negamax variant of alpha-beta pruning (ITERATIVE - no recursion)
//! - Iterative deepening for time management
//! - Transposition table for caching
//! - Move ordering for better pruning
//!
//! **Stack Overflow Fix**: Converted from recursive to iterative implementation
//! using explicit stack frames. This eliminates unbounded recursion and prevents
//! stack overflow at any search depth.
//!
//! ## Module Organization
//!
//! - `alphabeta` - Core alpha-beta search algorithm
//! - `quiescence` - Quiescence search to avoid horizon effect
//! - `ordering` - Move ordering heuristics
//! - `make_unmake` - Move making/unmaking utilities
//! - `iterative` - Iterative deepening wrapper

mod alphabeta;
mod iterative;
pub(crate) mod make_unmake;
mod move_picker;
mod ordering;
pub mod params;
mod quiescence;

pub use iterative::find_best_move;

/// Ad-hoc, fixed-time-budget nodes-searched measurement — not a criterion
/// harness (none exists in this crate), just enough to compare an engine
/// change's throughput before/after without adding a new dev-dependency for
/// a one-time measurement. Ignored by default like the other slow tests in
/// this crate (see `perft::tests`); run explicitly with:
///
/// `cargo test --release -p nimzovich_engine -- --ignored --nocapture bench_search_nodes_per_sec`
#[cfg(all(test, feature = "std"))]
mod bench {
    use crate::api::game::game_from_fen;

    const THINK_SECS: f32 = 2.0;

    const POSITIONS: &[(&str, &str)] = &[
        (
            "start",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ),
        (
            "kiwipete (tactical, capture-heavy)",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        ),
        (
            "pos5 (promotion-heavy)",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        ),
    ];

    #[test]
    #[ignore = "manual benchmark — run with --ignored --nocapture, ideally --release"]
    fn bench_search_nodes_per_sec() {
        let mut total_nodes: i64 = 0;
        let mut total_secs: f64 = 0.0;

        for (label, fen) in POSITIONS {
            let mut game = game_from_fen(fen);
            let start = std::time::Instant::now();
            let _best = super::find_best_move(&mut game, THINK_SECS, 1);
            let elapsed = start.elapsed();

            let nodes = game.calls;
            let nps = nodes as f64 / elapsed.as_secs_f64();
            println!(
                "{label:<36} nodes={nodes:>10}  time={:.3}s  nps={nps:>12.0}",
                elapsed.as_secs_f64()
            );

            total_nodes += nodes;
            total_secs += elapsed.as_secs_f64();
        }

        println!(
            "\nTOTAL: nodes={total_nodes}  time={total_secs:.3}s  nps={:.0}",
            total_nodes as f64 / total_secs
        );
    }
}
