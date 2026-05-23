use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::warn;

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
        if self.last_30_cpls.is_empty() { return f64::MAX; }
        self.last_30_cpls.iter().sum::<f64>() / self.last_30_cpls.len() as f64
    }

    pub fn rolling_avg_t1(&self) -> f64 {
        if self.last_30_t1s.is_empty() { return 0.0; }
        self.last_30_t1s.iter().sum::<f64>() / self.last_30_t1s.len() as f64
    }

    pub fn rolling_cpl_stddev(&self) -> f64 {
        if self.last_30_cpls.len() < 2 { return f64::MAX; }
        let mean = self.rolling_avg_cpl();
        let var = self.last_30_cpls.iter()
            .map(|c| (c - mean).powi(2))
            .sum::<f64>() / self.last_30_cpls.len() as f64;
        var.sqrt()
    }

    pub fn game_z_score(&self, game_cpl: f64) -> f64 {
        let stddev = self.rolling_cpl_stddev();
        if stddev < 1.0 { return 0.0; }
        (self.rolling_avg_cpl() - game_cpl) / stddev
    }
}

pub async fn load_stats(pool: &SqlitePool, pubkey: &str) -> PlayerStats {
    let row: Option<(i64, f64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT games_analysed, lifetime_cpl, last_30_cpls, last_30_t1s,
                flags_received, reviews_received
         FROM player_anticheat_stats WHERE pubkey = ?"
    )
    .bind(pubkey)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((games, cpl, cpls_json, t1s_json, flags, reviews)) => PlayerStats {
            pubkey: pubkey.to_string(),
            games_analysed: games as u32,
            lifetime_cpl: cpl,
            last_30_cpls: serde_json::from_str(&cpls_json).unwrap_or_default(),
            last_30_t1s: serde_json::from_str(&t1s_json).unwrap_or_default(),
            flags_received: flags as u32,
            reviews_received: reviews as u32,
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
    }
}

pub async fn update_stats(pool: &SqlitePool, side: &SideAnalysis) {
    let pubkey = &side.pubkey;
    let mut stats = load_stats(pool, pubkey).await;

    let game_cpl = side.signals.avg_cpl;
    let game_t1  = side.signals.t1_rate;

    stats.games_analysed += 1;
    stats.lifetime_cpl = (stats.lifetime_cpl * (stats.games_analysed - 1) as f64 + game_cpl)
        / stats.games_analysed as f64;

    stats.last_30_cpls.insert(0, game_cpl);
    stats.last_30_cpls.truncate(30);
    stats.last_30_t1s.insert(0, game_t1);
    stats.last_30_t1s.truncate(30);

    match side.verdict {
        Verdict::Flag   => stats.flags_received += 1,
        Verdict::Review => stats.reviews_received += 1,
        Verdict::Clean  => {}
    }

    let cpls_json = serde_json::to_string(&stats.last_30_cpls).unwrap_or_default();
    let t1s_json  = serde_json::to_string(&stats.last_30_t1s).unwrap_or_default();

    let result = sqlx::query(
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
               last_updated     = excluded.last_updated"#
    )
    .bind(&stats.pubkey)
    .bind(stats.games_analysed as i64)
    .bind(stats.lifetime_cpl)
    .bind(&cpls_json)
    .bind(&t1s_json)
    .bind(stats.flags_received as i64)
    .bind(stats.reviews_received as i64)
    .execute(pool)
    .await;

    if let Err(e) = result {
        warn!("[cross_game] failed to update stats for {pubkey}: {e}");
    }
}
