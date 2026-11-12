use crate::error::{AcError, AcResult};
#[cfg(test)]
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::types::{SideAnalysis, Verdict};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStats {
    pub pubkey: String,
    pub games_analysed: u32,
    pub lifetime_cpl: f64,
    pub last_30_cpls: Vec<f64>,
    pub last_30_t1s: Vec<f64>,
    pub flags_received: u32,
    pub reviews_received: u32,
}

impl PlayerStats {
    pub fn has_sufficient_history(&self) -> bool {
        self.last_30_cpls.len() >= 5
    }

    pub fn rolling_avg_cpl(&self) -> f64 {
        if self.last_30_cpls.is_empty() {
            return f64::MAX;
        }
        self.last_30_cpls.iter().sum::<f64>() / self.last_30_cpls.len() as f64
    }

    pub fn rolling_avg_t1(&self) -> f64 {
        if self.last_30_t1s.is_empty() {
            return 0.0;
        }
        self.last_30_t1s.iter().sum::<f64>() / self.last_30_t1s.len() as f64
    }

    pub fn rolling_cpl_stddev(&self) -> f64 {
        if self.last_30_cpls.len() < 2 {
            return f64::MAX;
        }
        let mean = self.rolling_avg_cpl();
        let var = self
            .last_30_cpls
            .iter()
            .map(|c| (c - mean).powi(2))
            .sum::<f64>()
            / self.last_30_cpls.len() as f64;
        var.sqrt()
    }

    pub fn game_z_score(&self, game_cpl: f64) -> f64 {
        let stddev = self.rolling_cpl_stddev();
        if stddev < 1.0 {
            return 0.0;
        }
        (self.rolling_avg_cpl() - game_cpl) / stddev
    }
}

pub async fn load_stats(pool: &SqlitePool, pubkey: &str) -> AcResult<PlayerStats> {
    load_stats_on(&mut *pool.acquire().await?, pubkey).await
}

pub(crate) async fn load_stats_on(
    connection: &mut SqliteConnection,
    pubkey: &str,
) -> AcResult<PlayerStats> {
    let row: Option<(i64, f64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT games_analysed, lifetime_cpl, last_30_cpls, last_30_t1s,
                flags_received, reviews_received
         FROM player_anticheat_stats WHERE pubkey = ?",
    )
    .bind(pubkey)
    .fetch_optional(connection)
    .await?;

    let count = |value| {
        u32::try_from(value)
            .map_err(|_| AcError::InvalidInput("invalid stored analysis count".into()))
    };
    Ok(match row {
        Some((games, cpl, cpls_json, t1s_json, flags, reviews)) => PlayerStats {
            pubkey: pubkey.to_string(),
            games_analysed: count(games)?,
            lifetime_cpl: cpl,
            last_30_cpls: serde_json::from_str(&cpls_json)?,
            last_30_t1s: serde_json::from_str(&t1s_json)?,
            flags_received: count(flags)?,
            reviews_received: count(reviews)?,
        },
        None => PlayerStats {
            pubkey: pubkey.to_string(),
            games_analysed: 0,
            lifetime_cpl: 0.0,
            last_30_cpls: vec![],
            last_30_t1s: vec![],
            flags_received: 0,
            reviews_received: 0,
        },
    })
}

pub async fn update_stats(pool: &SqlitePool, side: &SideAnalysis) -> AcResult<()> {
    let mut transaction = pool.begin().await?;
    update_stats_on(&mut transaction, side).await?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn update_stats_on(
    connection: &mut SqliteConnection,
    side: &SideAnalysis,
) -> AcResult<()> {
    let pubkey = &side.pubkey;
    let mut stats = load_stats_on(connection, pubkey).await?;

    let game_cpl = side.signals.avg_cpl;
    let game_t1 = side.signals.t1_rate;

    stats.games_analysed = stats
        .games_analysed
        .checked_add(1)
        .ok_or_else(|| AcError::InvalidInput("analysis counter exhausted".into()))?;
    stats.lifetime_cpl = (stats.lifetime_cpl * (stats.games_analysed - 1) as f64 + game_cpl)
        / stats.games_analysed as f64;

    stats.last_30_cpls.insert(0, game_cpl);
    stats.last_30_cpls.truncate(30);
    stats.last_30_t1s.insert(0, game_t1);
    stats.last_30_t1s.truncate(30);

    match side.verdict {
        Verdict::Flag => {
            stats.flags_received = stats
                .flags_received
                .checked_add(1)
                .ok_or_else(|| AcError::InvalidInput("flag counter exhausted".into()))?
        }
        Verdict::Review => {
            stats.reviews_received = stats
                .reviews_received
                .checked_add(1)
                .ok_or_else(|| AcError::InvalidInput("review counter exhausted".into()))?
        }
        Verdict::Clean => {}
    }

    let cpls_json = serde_json::to_string(&stats.last_30_cpls)?;
    let t1s_json = serde_json::to_string(&stats.last_30_t1s)?;

    sqlx::query(
        r#"INSERT INTO player_anticheat_stats
               (pubkey, games_analysed, lifetime_cpl, last_30_cpls, last_30_t1s,
                flags_received, reviews_received, last_updated)
           VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%s','now'))
           ON CONFLICT(pubkey) DO UPDATE SET
               games_analysed   = excluded.games_analysed,
               lifetime_cpl     = excluded.lifetime_cpl,
               last_30_cpls     = excluded.last_30_cpls,
               last_30_t1s      = excluded.last_30_t1s,
               flags_received   = excluded.flags_received,
               reviews_received = excluded.reviews_received,
               last_updated     = excluded.last_updated"#,
    )
    .bind(&stats.pubkey)
    .bind(stats.games_analysed as i64)
    .bind(stats.lifetime_cpl)
    .bind(&cpls_json)
    .bind(&t1s_json)
    .bind(stats.flags_received as i64)
    .bind(stats.reviews_received as i64)
    .execute(connection)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PlyEval, SignalValues, TimingSource, Verdict};

    async fn memory_pool() -> anyhow::Result<SqlitePool> {
        Ok({
            let pool = SqlitePool::connect("sqlite::memory:")
                .await
                .context("required operation value")?;
            sqlx::query(
                r#"CREATE TABLE player_anticheat_stats (
                pubkey          TEXT    PRIMARY KEY,
                games_analysed  INTEGER NOT NULL DEFAULT 0,
                lifetime_cpl    REAL    NOT NULL DEFAULT 0.0,
                last_30_cpls    TEXT    NOT NULL DEFAULT '[]',
                last_30_t1s     TEXT    NOT NULL DEFAULT '[]',
                flags_received  INTEGER NOT NULL DEFAULT 0,
                reviews_received INTEGER NOT NULL DEFAULT 0,
                last_updated    INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            )"#,
            )
            .execute(&pool)
            .await
            .context("required operation value")?;
            pool
        })
    }

    fn side(avg_cpl: f64) -> SideAnalysis {
        SideAnalysis {
            pubkey: "player1".into(),
            elo: 1500,
            signals: SignalValues {
                timing_anomaly: 0.0,
                cpl_vs_elo: 0.0,
                t1_rate: 0.0,
                avg_cpl,
                complex_ply_count: 10,
                blur_rate: 0.0,
                timing_source: TimingSource::Server,
            },
            weighted_score: 0.0,
            verdict: Verdict::Clean,
            ply_evals: Vec::<PlyEval>::new(),
        }
    }

    #[tokio::test]
    async fn no_history_reads_back_as_zeroed_stats() -> anyhow::Result<()> {
        let pool = memory_pool().await?;
        let stats = load_stats(&pool, "nobody").await?;
        assert_eq!(stats.games_analysed, 0);
        assert!(!stats.has_sufficient_history());
        Ok(())
    }

    #[tokio::test]
    async fn update_then_load_round_trips() -> anyhow::Result<()> {
        let pool = memory_pool().await?;
        update_stats(&pool, &side(50.0)).await?;

        let stats = load_stats(&pool, "player1").await?;
        assert_eq!(stats.games_analysed, 1);
        assert_eq!(stats.last_30_cpls, vec![50.0]);
        assert_eq!(stats.lifetime_cpl, 50.0);
        Ok(())
    }

    #[tokio::test]
    async fn z_score_flags_a_game_far_better_than_own_baseline() -> anyhow::Result<()> {
        let pool = memory_pool().await?;
        // Five prior games, consistently around 50 CPL (a steady baseline).
        for cpl in [48.0, 52.0, 49.0, 51.0, 50.0] {
            update_stats(&pool, &side(cpl)).await?;
        }

        let stats = load_stats(&pool, "player1").await?;
        assert!(stats.has_sufficient_history());

        // This game: 5 CPL — dramatically better than their own history.
        let z = stats.game_z_score(5.0);
        assert!(
            z > 2.0,
            "expected a high z-score for an outlier game, got {z}"
        );

        // A game right at their own average should read as unremarkable.
        let z_normal = stats.game_z_score(50.0);
        assert!(
            z_normal.abs() < 1.0,
            "expected ~0 z-score at baseline, got {z_normal}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn flags_and_reviews_counted_from_verdict() -> anyhow::Result<()> {
        let pool = memory_pool().await?;
        let mut flagged = side(10.0);
        flagged.verdict = Verdict::Flag;
        update_stats(&pool, &flagged).await?;

        let stats = load_stats(&pool, "player1").await?;
        assert_eq!(stats.flags_received, 1);
        assert_eq!(stats.reviews_received, 0);
        Ok(())
    }
}
