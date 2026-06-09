use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::AcConfig;
use crate::error::AcResult;
use crate::types::GameRecord;
use crate::analyse_game;

pub type JobId = String;

pub struct AnalysisJob {
    pub game: GameRecord,
}

/// Bounded channel sender for queuing games for analysis.
#[derive(Clone)]
pub struct AnalysisQueue {
    tx: mpsc::Sender<AnalysisJob>,
}

impl AnalysisQueue {
    pub async fn enqueue(&self, game: GameRecord) -> AcResult<JobId> {
        let job_id = game.game_id.clone();
        self.tx
            .try_send(AnalysisJob { game })
            .map_err(|_| crate::error::AcError::QueueFull)?;
        Ok(job_id)
    }
}

pub fn spawn_workers(cfg: Arc<AcConfig>, pool: sqlx::SqlitePool) -> AnalysisQueue {
    let (tx, rx) = mpsc::channel::<AnalysisJob>(cfg.queue_capacity);
    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    for worker_id in 0..cfg.worker_count {
        let cfg = Arc::clone(&cfg);
        let pool = pool.clone();
        let rx = Arc::clone(&rx);

        tokio::spawn(async move {
            info!("[anticheat worker {worker_id}] starting");
            loop {
                let job = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };
                match job {
                    None => {
                        info!("[anticheat worker {worker_id}] queue closed, shutting down");
                        break;
                    }
                    Some(job) => {
                        let game_id = job.game.game_id.clone();
                        info!("[anticheat worker {worker_id}] analysing game {game_id}");
                        match analyse_game(job.game, &cfg).await {
                            Ok(report) => {
                                if let Err(e) = crate::report::store::save_report(&pool, &report, &cfg).await {
                                    error!("[anticheat worker {worker_id}] save_report failed for {game_id}: {e}");
                                }
                                let _ = sqlx::query(
                                    "DELETE FROM anticheat_queue WHERE game_id = ?"
                                )
                                .bind(&game_id)
                                .execute(&pool)
                                .await;
                            }
                            Err(e) => {
                                warn!("[anticheat worker {worker_id}] analysis failed for {game_id}: {e}");
                                let e_str = e.to_string();
                                let _ = sqlx::query(
                                    "UPDATE anticheat_queue SET attempts = attempts + 1, last_error = ? WHERE game_id = ?"
                                )
                                .bind(&e_str)
                                .bind(&game_id)
                                .execute(&pool)
                                .await;
                            }
                        }
                    }
                }
            }
        });
    }

    AnalysisQueue { tx }
}
