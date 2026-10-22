use std::sync::Arc;
use tracing::info;

use xfchess_anticheat::config::AcConfig;
use xfchess_anticheat::engine::job_queue::{spawn_workers, AnalysisQueue};

pub fn spawn_anticheat_workers(pool: sqlx::SqlitePool) -> anyhow::Result<AnalysisQueue> {
    let cfg = Arc::new(AcConfig::from_env());
    info!(
        "[anticheat] starting {} Stockfish worker(s) at depth {}",
        cfg.worker_count, cfg.analysis_depth
    );
    Ok(spawn_workers(cfg, pool)?)
}
