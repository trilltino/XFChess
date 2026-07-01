//! Durable SQLite-backed job queue (Production Reality Plan, WS-A).
//!
//! For one-shot work that has **no durable backing of its own** — email sends,
//! anti-cheat analyses, future webhooks. If the process crashes between "we decided
//! to do X" and "X happened", the job survives in SQLite and is retried.
//!
//! Deliberately **not** used for settlement / prize distribution: those workers
//! re-derive their work from on-chain state every tick (the chain is the durable
//! queue), and mirroring that into a second store would create a split-brain.
//!
//! Semantics:
//! - **Idempotent enqueue** via optional `dedupe_key` (UNIQUE; re-enqueue is a no-op).
//! - **At-least-once execution** — handlers must themselves be idempotent.
//! - **Bounded retries** with exponential backoff + jitter; exhausted jobs land in
//!   the DLQ (`status='dead'`) for review (see runbooks).
//! - **Stale-claim recovery** — jobs claimed by a process that died are reclaimed
//!   after `STALE_CLAIM_SECS`.
//!
//! Single-writer discipline: one poller task per process; SQLite's WAL handles the
//! rest at our scale.

use serde::{de::DeserializeOwned, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Poll interval for due jobs.
const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// A `running` job older than this is considered orphaned (worker died) and reclaimed.
const STALE_CLAIM_SECS: i64 = 300;
/// Base backoff (doubles per attempt) capped at `BACKOFF_CAP_SECS`.
const BACKOFF_BASE_SECS: u64 = 30;
const BACKOFF_CAP_SECS: u64 = 3600;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Exponential backoff with ±20% jitter so retry storms can't synchronize.
fn backoff_secs(attempt: u32) -> i64 {
    let base = BACKOFF_BASE_SECS.saturating_mul(1u64 << attempt.min(6)); // 30s,1m,2m,4m,8m,16m,32m cap
    let capped = base.min(BACKOFF_CAP_SECS);
    // Cheap jitter without pulling in `rand`: subsecond nanos are effectively random here.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let jitter = (capped / 5).max(1);
    (capped - jitter / 2 + (nanos % jitter)) as i64
}

/// A claimed job handed to a handler.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub payload: String,
    pub attempts: i64,
}

impl Job {
    /// Deserialize the JSON payload into a concrete type.
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.payload)
    }
}

/// Boxed async handler: `Ok(())` = done, `Err(msg)` = retry (until max_attempts).
pub type Handler =
    Arc<dyn Fn(Job) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

/// Enqueue a job. `dedupe_key = Some(..)` makes the enqueue idempotent — a second
/// call with the same key is a silent no-op (returns Ok(None)).
pub async fn enqueue<P: Serialize>(
    pool: &SqlitePool,
    kind: &str,
    payload: &P,
    dedupe_key: Option<&str>,
) -> Result<Option<i64>, sqlx::Error> {
    let payload = serde_json::to_string(payload).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let ts = now();
    let res = sqlx::query(
        r#"INSERT INTO jobs (kind, payload, dedupe_key, run_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(dedupe_key) DO NOTHING"#,
    )
    .bind(kind)
    .bind(&payload)
    .bind(dedupe_key)
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None); // deduped
    }
    Ok(Some(res.last_insert_rowid()))
}

/// Claim the next due pending job (oldest first). Also reclaims stale `running`
/// jobs whose worker died. Returns None when nothing is due.
async fn claim_next(pool: &SqlitePool) -> Result<Option<Job>, sqlx::Error> {
    let ts = now();

    // Recover orphaned claims first (process died mid-job).
    sqlx::query(
        r#"UPDATE jobs SET status='pending', claimed_at=NULL, updated_at=?
           WHERE status='running' AND claimed_at < ?"#,
    )
    .bind(ts)
    .bind(ts - STALE_CLAIM_SECS)
    .execute(pool)
    .await?;

    // Atomically claim one due job. SQLite's RETURNING makes this race-free enough
    // for our single-poller-per-process model.
    let row: Option<(i64, String, String, i64)> = sqlx::query_as(
        r#"UPDATE jobs SET status='running', claimed_at=?, updated_at=?
           WHERE id = (
               SELECT id FROM jobs WHERE status='pending' AND run_at <= ?
               ORDER BY run_at ASC LIMIT 1
           )
           RETURNING id, kind, payload, attempts"#,
    )
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, kind, payload, attempts)| Job { id, kind, payload, attempts }))
}

async fn mark_done(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET status='done', updated_at=? WHERE id=?")
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Record a failure: schedule a retry with backoff, or move to the DLQ when
/// attempts are exhausted.
async fn mark_failed(pool: &SqlitePool, job: &Job, err: &str) -> Result<(), sqlx::Error> {
    let ts = now();
    let attempts = job.attempts + 1;
    let row: Option<(i64,)> = sqlx::query_as("SELECT max_attempts FROM jobs WHERE id=?")
        .bind(job.id)
        .fetch_optional(pool)
        .await?;
    let max_attempts = row.map(|(m,)| m).unwrap_or(5);

    if attempts >= max_attempts {
        warn!("[queue] job {} kind={} DEAD after {} attempts: {}", job.id, job.kind, attempts, err);
        sqlx::query(
            "UPDATE jobs SET status='dead', attempts=?, last_error=?, updated_at=? WHERE id=?",
        )
        .bind(attempts)
        .bind(err)
        .bind(ts)
        .bind(job.id)
        .execute(pool)
        .await?;
    } else {
        let delay = backoff_secs(attempts as u32);
        info!(
            "[queue] job {} kind={} failed (attempt {}/{}), retry in {}s: {}",
            job.id, job.kind, attempts, max_attempts, delay, err
        );
        sqlx::query(
            r#"UPDATE jobs SET status='pending', attempts=?, last_error=?, run_at=?,
                             claimed_at=NULL, updated_at=? WHERE id=?"#,
        )
        .bind(attempts)
        .bind(err)
        .bind(ts + delay)
        .bind(ts)
        .bind(job.id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Number of jobs currently in the DLQ (exposed as a metric; alert when > 0).
pub async fn dead_count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE status='dead'")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Registry of job-kind → handler, then `spawn` the poller.
#[derive(Default)]
pub struct QueueWorker {
    handlers: HashMap<String, Handler>,
}

impl QueueWorker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for a job kind. Handlers must be idempotent
    /// (at-least-once execution).
    pub fn register<F, Fut>(mut self, kind: &str, f: F) -> Self
    where
        F: Fn(Job) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.handlers
            .insert(kind.to_string(), Arc::new(move |job| Box::pin(f(job))));
        self
    }

    /// Spawn the polling loop. One per process.
    pub fn spawn(self, pool: SqlitePool) {
        let handlers = Arc::new(self.handlers);
        tokio::spawn(async move {
            info!("[queue] worker started ({} handler(s))", handlers.len());
            loop {
                match claim_next(&pool).await {
                    Ok(Some(job)) => {
                        let outcome = match handlers.get(&job.kind) {
                            Some(h) => h(job.clone()).await,
                            None => Err(format!("no handler registered for kind '{}'", job.kind)),
                        };
                        let res = match outcome {
                            Ok(()) => mark_done(&pool, job.id).await,
                            Err(e) => mark_failed(&pool, &job, &e).await,
                        };
                        if let Err(e) = res {
                            error!("[queue] failed to update job {}: {}", job.id, e);
                        }
                        // Immediately look for the next due job (drain bursts).
                        continue;
                    }
                    Ok(None) => {}
                    Err(e) => error!("[queue] claim error: {}", e),
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        // raw_sql executes the multi-statement migration script as-is.
        sqlx::raw_sql(include_str!("../../migrations/019_job_queue.sql"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn enqueue_and_claim_roundtrip() {
        let pool = test_pool().await;
        let id = enqueue(&pool, "test.echo", &serde_json::json!({"x":1}), None)
            .await
            .unwrap();
        assert!(id.is_some());

        let job = claim_next(&pool).await.unwrap().expect("job due");
        assert_eq!(job.kind, "test.echo");
        mark_done(&pool, job.id).await.unwrap();
        assert!(claim_next(&pool).await.unwrap().is_none(), "no more jobs");
    }

    #[tokio::test]
    async fn dedupe_key_makes_enqueue_idempotent() {
        let pool = test_pool().await;
        let first = enqueue(&pool, "test.mail", &serde_json::json!({}), Some("mail:alice"))
            .await
            .unwrap();
        let second = enqueue(&pool, "test.mail", &serde_json::json!({}), Some("mail:alice"))
            .await
            .unwrap();
        assert!(first.is_some());
        assert!(second.is_none(), "duplicate enqueue must be a no-op");
    }

    #[tokio::test]
    async fn failure_backs_off_then_dlqs() {
        let pool = test_pool().await;
        enqueue(&pool, "test.fail", &serde_json::json!({}), None)
            .await
            .unwrap();
        // Force max_attempts=2 for the test.
        sqlx::query("UPDATE jobs SET max_attempts=2")
            .execute(&pool)
            .await
            .unwrap();

        let job = claim_next(&pool).await.unwrap().unwrap();
        mark_failed(&pool, &job, "boom").await.unwrap();
        // Retry scheduled in the future → not claimable now.
        assert!(claim_next(&pool).await.unwrap().is_none());

        // Make it due and fail again → DLQ.
        sqlx::query("UPDATE jobs SET run_at=0 WHERE status='pending'")
            .execute(&pool)
            .await
            .unwrap();
        let job = claim_next(&pool).await.unwrap().unwrap();
        mark_failed(&pool, &job, "boom again").await.unwrap();

        assert_eq!(dead_count(&pool).await.unwrap(), 1, "job must land in DLQ");
    }

    #[test]
    fn backoff_grows_and_caps() {
        let a1 = backoff_secs(1);
        let a6 = backoff_secs(6);
        assert!(a1 >= 24 && a1 <= 72, "attempt 1 ≈ 60s±jitter, got {a1}");
        assert!(a6 <= BACKOFF_CAP_SECS as i64 + 1, "capped, got {a6}");
    }
}
