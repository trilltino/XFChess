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
/// Unix timestamp (seconds) of the start of the most recent scan tick — a
/// liveness signal distinct from the tick counter itself: a worker that
/// panics mid-loop stops advancing this even though the process is still up.
/// Read by `GET /admin/tasks/status`.
pub static SETTLEMENT_LAST_TICK_UNIX: AtomicU64 = AtomicU64::new(0);
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
/// Active, wagered games the worker found still undelegated well past a
/// normal create/join → delegate handshake and successfully redelegated
/// (see `STALE_UNDELEGATED_SECS`/`redelegate_stale_game` in
/// settlement_worker.rs) — the client-side delegation attempt failed or
/// never ran, so this is the automatic recovery path for it.
pub static SETTLEMENT_REDELEGATE_RETRIED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Redelegate attempts (above) that themselves failed to build or submit.
pub static SETTLEMENT_REDELEGATE_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Games `force_undelegate_after_timeout` recovered without the ER's
/// cooperation, whose escrow release (`governance_ix::recover_stuck_delegation`)
/// the worker then attempted automatically but could not complete (e.g.
/// `DISPUTE_AUTHORITY_KEYPAIR` unset, or the on-chain call itself failed) —
/// a true fallback state needing the manual `POST
/// /admin/dispute/recover_stuck_delegation` step. Should normally stay at 0;
/// see `STUCK_DELEGATION_AUTO_RECOVERED_TOTAL` for the (expected) common case.
pub static FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Games whose escrow the worker force-undelegated *and* automatically
/// released via `recover_stuck_delegation` in the same tick, closing the
/// ER-unavailability escape hatch end-to-end with no human step.
pub static STUCK_DELEGATION_AUTO_RECOVERED_TOTAL: AtomicU64 = AtomicU64::new(0);

// ── ER real-time subscription (Triton WS pubsub, see tasks/er_watch.rs) ───────
/// 1 if the `accountSubscribe` WS pubsub connection to Triton is currently up,
/// 0 if it's down/reconnecting — in which case `settlement_worker` has fallen
/// back to poll-only staleness detection (same behavior as before this
/// existed, never a regression, just less timely).
pub static ER_SUBSCRIPTION_CONNECTED: AtomicU64 = AtomicU64::new(0);
/// Unix timestamp (seconds) of the most recent account-update push received
/// from any watched, ER-delegated Game PDA. 0 if none received yet.
pub static ER_LAST_PUSH_UNIX: AtomicU64 = AtomicU64::new(0);

// ── Tournament scheduler / prize distributor ──────────────────────────────────
/// Unix timestamp (seconds) of the most recent tournament-scheduler
/// (round-advancement) loop pass.
pub static TOURNAMENT_SCHEDULER_LAST_TICK_UNIX: AtomicU64 = AtomicU64::new(0);
/// Unix timestamp (seconds) of the most recent prize-distributor loop pass.
pub static PRIZE_DISTRIBUTOR_LAST_TICK_UNIX: AtomicU64 = AtomicU64::new(0);

// ── Time-check crank (ER) ─────────────────────────────────────────────────────
/// `schedule_time_check` calls submitted successfully after `delegate_game`.
pub static TIME_CHECK_SCHEDULED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// `schedule_time_check` calls that failed to build or submit. Best-effort —
/// delegation itself already succeeded, so these are logged/counted, not
/// retried inline.
pub static TIME_CHECK_SCHEDULE_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// `cancel_time_check` calls submitted successfully alongside `undelegate_game`.
pub static TIME_CHECK_CANCELLED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// `cancel_time_check` calls that failed to build or submit. A stray
/// scheduled task on an undelegated/foreign account is expected to fail
/// harmlessly on its own next tick, so this is a cleanliness signal, not a
/// correctness one.
pub static TIME_CHECK_CANCEL_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);

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

// ── Exchange rates (RateCache) ──────────────────────────────────────────────
/// Both the primary and secondary rate sources failed on the same refresh —
/// only stale cache (or nothing) was available.
pub static RATES_FETCH_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// A fetched rate was discarded for falling outside the sanity bounds
/// (a decimal/parsing bug or a genuinely broken upstream feed).
pub static RATES_SANITY_REJECTED_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Primary and secondary sources both returned usable rates but disagreed
/// beyond the alert threshold — logged and counted, primary still served.
pub static RATES_SOURCE_DIVERGENCE_TOTAL: AtomicU64 = AtomicU64::new(0);

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
         # HELP xfchess_settlement_last_tick_unix Unix timestamp of the most recent settlement worker tick\n\
         # TYPE xfchess_settlement_last_tick_unix gauge\n\
         xfchess_settlement_last_tick_unix {}\n\
         # HELP xfchess_tournament_scheduler_last_tick_unix Unix timestamp of the most recent tournament scheduler tick\n\
         # TYPE xfchess_tournament_scheduler_last_tick_unix gauge\n\
         xfchess_tournament_scheduler_last_tick_unix {}\n\
         # HELP xfchess_prize_distributor_last_tick_unix Unix timestamp of the most recent prize distributor tick\n\
         # TYPE xfchess_prize_distributor_last_tick_unix gauge\n\
         xfchess_prize_distributor_last_tick_unix {}\n\
         # HELP xfchess_settlement_stale_delegated_gauge Delegated games with no on-chain activity beyond the expected window (possible stuck ER delegation)\n\
         # TYPE xfchess_settlement_stale_delegated_gauge gauge\n\
         xfchess_settlement_stale_delegated_gauge {}\n\
         # HELP xfchess_er_subscription_connected Whether the Triton WS pubsub account-subscribe stream is currently connected (1) or degraded to poll-only (0)\n\
         # TYPE xfchess_er_subscription_connected gauge\n\
         xfchess_er_subscription_connected {}\n\
         # HELP xfchess_er_last_push_unix Unix timestamp of the most recent ER account-update push received\n\
         # TYPE xfchess_er_last_push_unix gauge\n\
         xfchess_er_last_push_unix {}\n\
         # HELP xfchess_settlement_redelegate_retried_total Stuck non-delegated wagered games successfully auto-redelegated\n\
         # TYPE xfchess_settlement_redelegate_retried_total counter\n\
         xfchess_settlement_redelegate_retried_total {}\n\
         # HELP xfchess_settlement_redelegate_failed_total Auto-redelegate attempts that failed\n\
         # TYPE xfchess_settlement_redelegate_failed_total counter\n\
         xfchess_settlement_redelegate_failed_total {}\n\
         # HELP xfchess_force_undelegated_awaiting_recovery_total Games force-undelegated after ER-unavailability timeout whose automatic escrow recovery failed and still need the manual admin step\n\
         # TYPE xfchess_force_undelegated_awaiting_recovery_total counter\n\
         xfchess_force_undelegated_awaiting_recovery_total {}\n\
         # HELP xfchess_stuck_delegation_auto_recovered_total Games force-undelegated after ER-unavailability timeout whose escrow the worker released automatically\n\
         # TYPE xfchess_stuck_delegation_auto_recovered_total counter\n\
         xfchess_stuck_delegation_auto_recovered_total {}\n\
         # HELP xfchess_time_check_scheduled_total schedule_time_check calls submitted successfully\n\
         # TYPE xfchess_time_check_scheduled_total counter\n\
         xfchess_time_check_scheduled_total {}\n\
         # HELP xfchess_time_check_schedule_failed_total schedule_time_check calls that failed\n\
         # TYPE xfchess_time_check_schedule_failed_total counter\n\
         xfchess_time_check_schedule_failed_total {}\n\
         # HELP xfchess_time_check_cancelled_total cancel_time_check calls submitted successfully\n\
         # TYPE xfchess_time_check_cancelled_total counter\n\
         xfchess_time_check_cancelled_total {}\n\
         # HELP xfchess_time_check_cancel_failed_total cancel_time_check calls that failed\n\
         # TYPE xfchess_time_check_cancel_failed_total counter\n\
         xfchess_time_check_cancel_failed_total {}\n\
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
         xfchess_linkage_hard_blocked_total {}\n\
         # HELP xfchess_rates_fetch_failed_total Refreshes where both rate sources failed\n\
         # TYPE xfchess_rates_fetch_failed_total counter\n\
         xfchess_rates_fetch_failed_total {}\n\
         # HELP xfchess_rates_sanity_rejected_total Fetched rates discarded for failing sanity bounds\n\
         # TYPE xfchess_rates_sanity_rejected_total counter\n\
         xfchess_rates_sanity_rejected_total {}\n\
         # HELP xfchess_rates_source_divergence_total Refreshes where primary/secondary rate sources disagreed beyond threshold\n\
         # TYPE xfchess_rates_source_divergence_total counter\n\
         xfchess_rates_source_divergence_total {}\n",
        c(&SETTLEMENT_TICKS_TOTAL),
        c(&SETTLEMENT_TICK_MILLIS),
        c(&SETTLEMENT_GAMES_SCANNED_TOTAL),
        c(&SETTLEMENT_FINALIZED_TOTAL),
        c(&SETTLEMENT_UNDELEGATED_TOTAL),
        c(&SETTLEMENT_RPC_CALLS_TOTAL),
        c(&SETTLEMENT_LAST_TICK_UNIX),
        c(&TOURNAMENT_SCHEDULER_LAST_TICK_UNIX),
        c(&PRIZE_DISTRIBUTOR_LAST_TICK_UNIX),
        c(&SETTLEMENT_STALE_DELEGATED_GAUGE),
        c(&ER_SUBSCRIPTION_CONNECTED),
        c(&ER_LAST_PUSH_UNIX),
        c(&SETTLEMENT_REDELEGATE_RETRIED_TOTAL),
        c(&SETTLEMENT_REDELEGATE_FAILED_TOTAL),
        c(&FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL),
        c(&STUCK_DELEGATION_AUTO_RECOVERED_TOTAL),
        c(&TIME_CHECK_SCHEDULED_TOTAL),
        c(&TIME_CHECK_SCHEDULE_FAILED_TOTAL),
        c(&TIME_CHECK_CANCELLED_TOTAL),
        c(&TIME_CHECK_CANCEL_FAILED_TOTAL),
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
        c(&RATES_FETCH_FAILED_TOTAL),
        c(&RATES_SANITY_REJECTED_TOTAL),
        c(&RATES_SOURCE_DIVERGENCE_TOTAL),
    )
}
