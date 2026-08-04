//! Spawns the anti-cheat analysis queue workers at server startup.

use std::sync::Arc;
use tracing::info;

use xfchess_anticheat::config::AcConfig;
use xfchess_anticheat::engine::job_queue::{spawn_workers, AnalysisQueue};

/// Initialise the anti-cheat job queue and return the sender handle.
/// Call once from `spawn_background_tasks`.
pub fn spawn_anticheat_workers(pool: sqlx::SqlitePool) -> AnalysisQueue {
    let cfg = Arc::new(AcConfig::from_env());
    info!(
        "[anticheat] starting {} Stockfish worker(s) at depth {}",
        cfg.worker_count, cfg.analysis_depth
    );
    spawn_workers(cfg, pool)
}
