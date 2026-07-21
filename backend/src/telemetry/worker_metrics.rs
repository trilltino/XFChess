//! Static Prometheus counters for background workers.
//!
//! Plain atomics rather than the `Metrics` registry because workers run in
//! detached tasks where threading `Arc<RwLock<Metrics>>` through every spawn
//! adds noise for no benefit. Rendered by [`render_prometheus`], which the
//! `/metrics` endpoint appends to its output.

use std::sync::atomic::{AtomicU64, Ordering};

// ── Settlement worker ─────────────────────────────────────────────────────────
pub static SETTLEMENT_TICKS_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Duration of the most recent scan tick, in milliseconds.
pub static SETTLEMENT_TICK_MILLIS: AtomicU64 = AtomicU64::new(0);
pub static SETTLEMENT_GAMES_SCANNED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static SETTLEMENT_FINALIZED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static SETTLEMENT_UNDELEGATED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Batched `getMultipleAccounts` calls issued by the settlement worker.
pub static SETTLEMENT_RPC_CALLS_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Currently-delegated games with no on-chain activity for longer than
/// `STALE_DELEGATION_SECS` (settlement_worker.rs) — a signal that the ER
/// validator may not be committing/undelegating as expected. This is a
/// monitoring signal only: XFChess has no way to force a delegated game back
/// to the base layer without the ER's cooperation (see the persistency
/// roadmap's MagicBlock section), so the response to this metric firing is
/// operational (page, investigate, contact MagicBlock), not automatic.
pub static SETTLEMENT_STALE_DELEGATED_GAUGE: AtomicU64 = AtomicU64::new(0);

// ── Anti-cheat enqueue ────────────────────────────────────────────────────────
pub static ANTICHEAT_ENQUEUED_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static ANTICHEAT_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Free games whose T0 timing screen came back clean — no engine analysis.
pub static ANTICHEAT_SCREENED_OUT_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Jobs sitting in the in-memory analysis queue (sampled at enqueue time).
pub static ANTICHEAT_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
/// Sides whose client think-time telemetry was discarded for exceeding the
/// server-observed wall-clock budget (a tamper signal).
pub static TELEMETRY_DISCARDED_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── Sybil / multi-accounting ──────────────────────────────────────────────────
/// Wallets surfaced for review by the linkage/collusion signals (soft).
pub static LINKAGE_FLAGGED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Prize-entry registrations refused for a hard linkage/KYC collision.
pub static LINKAGE_HARD_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── Prize distribution ────────────────────────────────────────────────────────
pub static PRIZE_DISTRIBUTED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Ticks where a distribution was deferred waiting for anti-cheat analysis.
pub static PRIZE_DISTRIBUTION_HELD_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Prize places skipped because the winner had a flagged verdict.
pub static PRIZE_DISTRIBUTION_FLAGGED_TOTAL: AtomicU64 = AtomicU64::new(0);

pub fn render_prometheus() -> String {
    let c = |v: &AtomicU64| v.load(Ordering::Relaxed);
    format!(
        "# HELP xfchess_settlement_ticks_total Settlement worker scan ticks\n\
         # TYPE xfchess_settlement_ticks_total counter\n\
         xfchess_settlement_ticks_total {}\n\
         # HELP xfchess_settlement_tick_millis Duration of last settlement tick (ms)\n\
         # TYPE xfchess_settlement_tick_millis gauge\n\
         xfchess_settlement_tick_millis {}\n\
         # HELP xfchess_settlement_games_scanned_total Active games inspected on-chain\n\
         # TYPE xfchess_settlement_games_scanned_total counter\n\
         xfchess_settlement_games_scanned_total {}\n\
         # HELP xfchess_settlement_finalized_total Games auto-finalized by the worker\n\
         # TYPE xfchess_settlement_finalized_total counter\n\
         xfchess_settlement_finalized_total {}\n\
         # HELP xfchess_settlement_undelegated_total Games auto-undelegated from ER\n\
         # TYPE xfchess_settlement_undelegated_total counter\n\
         xfchess_settlement_undelegated_total {}\n\
         # HELP xfchess_settlement_rpc_calls_total Batched account-fetch RPC calls\n\
         # TYPE xfchess_settlement_rpc_calls_total counter\n\
         xfchess_settlement_rpc_calls_total {}\n\
         # HELP xfchess_settlement_stale_delegated_gauge Delegated games with no on-chain activity beyond the expected window (possible stuck ER delegation)\n\
         # TYPE xfchess_settlement_stale_delegated_gauge gauge\n\
         xfchess_settlement_stale_delegated_gauge {}\n\
         # HELP xfchess_anticheat_enqueued_total Games queued for anti-cheat analysis\n\
         # TYPE xfchess_anticheat_enqueued_total counter\n\
         xfchess_anticheat_enqueued_total {}\n\
         # HELP xfchess_anticheat_dropped_total Games dropped because the analysis queue was full\n\
         # TYPE xfchess_anticheat_dropped_total counter\n\
         xfchess_anticheat_dropped_total {}\n\
         # HELP xfchess_anticheat_screened_out_total Free games cleared by the T0 screen without engine analysis\n\
         # TYPE xfchess_anticheat_screened_out_total counter\n\
         xfchess_anticheat_screened_out_total {}\n\
         # HELP xfchess_anticheat_queue_depth Jobs in the in-memory analysis queue\n\
         # TYPE xfchess_anticheat_queue_depth gauge\n\
         xfchess_anticheat_queue_depth {}\n\
         # HELP xfchess_telemetry_discarded_total Sides whose client think-time telemetry failed the wall-clock budget\n\
         # TYPE xfchess_telemetry_discarded_total counter\n\
         xfchess_telemetry_discarded_total {}\n\
         # HELP xfchess_anticheat_analyses_total Completed Stockfish analyses\n\
         # TYPE xfchess_anticheat_analyses_total counter\n\
         xfchess_anticheat_analyses_total {}\n\
         # HELP xfchess_anticheat_analysis_failures_total Stockfish analyses that errored\n\
         # TYPE xfchess_anticheat_analysis_failures_total counter\n\
         xfchess_anticheat_analysis_failures_total {}\n\
         # HELP xfchess_anticheat_analysis_millis Duration of the most recent analysis (ms)\n\
         # TYPE xfchess_anticheat_analysis_millis gauge\n\
         xfchess_anticheat_analysis_millis {}\n\
         # HELP xfchess_prize_distributed_total Tournaments whose prizes were auto-distributed\n\
         # TYPE xfchess_prize_distributed_total counter\n\
         xfchess_prize_distributed_total {}\n\
         # HELP xfchess_prize_distribution_held_total Distribution ticks deferred for pending anti-cheat analysis\n\
         # TYPE xfchess_prize_distribution_held_total counter\n\
         xfchess_prize_distribution_held_total {}\n\
         # HELP xfchess_prize_distribution_flagged_total Prize places withheld due to flagged anti-cheat verdicts\n\
         # TYPE xfchess_prize_distribution_flagged_total counter\n\
         xfchess_prize_distribution_flagged_total {}\n\
         # HELP xfchess_linkage_flagged_total Wallets surfaced for review by Sybil-linkage signals\n\
         # TYPE xfchess_linkage_flagged_total counter\n\
         xfchess_linkage_flagged_total {}\n\
         # HELP xfchess_linkage_hard_blocked_total Prize registrations refused for a hard linkage/KYC collision\n\
         # TYPE xfchess_linkage_hard_blocked_total counter\n\
         xfchess_linkage_hard_blocked_total {}\n",
        c(&SETTLEMENT_TICKS_TOTAL),
        c(&SETTLEMENT_TICK_MILLIS),
        c(&SETTLEMENT_GAMES_SCANNED_TOTAL),
        c(&SETTLEMENT_FINALIZED_TOTAL),
        c(&SETTLEMENT_UNDELEGATED_TOTAL),
        c(&SETTLEMENT_RPC_CALLS_TOTAL),
        c(&SETTLEMENT_STALE_DELEGATED_GAUGE),
        c(&ANTICHEAT_ENQUEUED_TOTAL),
        c(&ANTICHEAT_DROPPED_TOTAL),
        c(&ANTICHEAT_SCREENED_OUT_TOTAL),
        c(&ANTICHEAT_QUEUE_DEPTH),
        c(&TELEMETRY_DISCARDED_TOTAL),
        c(&xfchess_anticheat::metrics::ANALYSES_TOTAL),
        c(&xfchess_anticheat::metrics::ANALYSIS_FAILURES_TOTAL),
        c(&xfchess_anticheat::metrics::ANALYSIS_MILLIS_LAST),
        c(&PRIZE_DISTRIBUTED_TOTAL),
        c(&PRIZE_DISTRIBUTION_HELD_TOTAL),
        c(&PRIZE_DISTRIBUTION_FLAGGED_TOTAL),
        c(&LINKAGE_FLAGGED_TOTAL),
        c(&LINKAGE_HARD_BLOCKED_TOTAL),
    )
}
