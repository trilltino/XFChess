//! Shared data types and state for the matchmaking system.
//!
//! [`MatchmakingTicket`] represents a player waiting in the queue,
//! [`MatchResult`] is the payload handed back once they are paired, and
//! [`SharedMatchmakingState`] bundles the queue, pending matches, and ELO
//! cache behind `Arc<Mutex<_>>` so Axum handlers can share them.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{error, info};

/// Matchmaking ticket representing a player waiting for a match.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchmakingTicket {
    /// Player's wallet public key.
    pub pubkey: String,
    /// Player's ELO rating.
    pub elo: u32,
    /// Unix timestamp when the player joined the queue.
    pub joined_at: u64,
}

/// Match result returned when a player is matched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchResult {
    /// The game ID for the matched game.
    pub game_id: u64,
    /// Opponent's wallet public key.
    pub opponent: String,
    /// Whether the player plays as white.
    pub is_white: bool,
    /// Unix timestamp when the match was made — lets a background sweep
    /// evict entries a player never came back to retrieve (e.g. a crash
    /// right after being paired), instead of keeping them forever.
    pub matched_at: u64,
}

/// Shared state for the matchmaking system.
///
/// Contains the player queue, pending match results, and the ELO cache
/// used to look up on-chain ratings.
#[derive(Clone)]
pub struct SharedMatchmakingState {
    /// Queue of players waiting for matches.
    pub queue: Arc<Mutex<Vec<MatchmakingTicket>>>,
    /// Map from pubkey to match result (one-time retrieval).
    pub matches: Arc<Mutex<HashMap<String, MatchResult>>>,
    /// ELO cache for fetching player ratings.
    pub elo_cache: Arc<crate::signing::EloCache>,
    /// Backing store for the queue/matches so a backend restart can reload
    /// them instead of silently dropping every queued player and pending
    /// match (`matchmaking_queue` / `matchmaking_matches`, migration 022).
    /// The in-memory maps above stay the hot path; this is write-through.
    pub pool: sqlx::SqlitePool,
}

impl SharedMatchmakingState {
    /// Build matchmaking state sharing the app's already-configured ELO
    /// cache, so it queries the same RPC/program the rest of the backend
    /// does instead of a second, independent devnet-hardcoded cache.
    pub fn new(elo_cache: Arc<crate::signing::EloCache>, pool: sqlx::SqlitePool) -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            matches: Arc::new(Mutex::new(HashMap::new())),
            elo_cache,
            pool,
        }
    }

    /// Reload the queue and pending matches from SQLite. Call once at
    /// startup (before the matchmaking loop starts ticking) so a backend
    /// restart resumes from where it left off instead of losing every
    /// queued player and pending match. Any rows stale enough to be swept
    /// by `run_matchmaking_service` are cleaned up on its next tick, so
    /// this doesn't need to re-check staleness itself.
    pub async fn hydrate(&self) {
        match sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT pubkey, elo, joined_at FROM matchmaking_queue",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => {
                let count = rows.len();
                let mut queue = self
                    .queue
                    .lock()
                    .expect("Mutex lock should not be poisoned");
                queue.extend(
                    rows.into_iter()
                        .map(|(pubkey, elo, joined_at)| MatchmakingTicket {
                            pubkey,
                            elo: elo as u32,
                            joined_at: joined_at as u64,
                        }),
                );
                info!("[Matchmaking] Hydrated {count} queued ticket(s) from SQLite");
            }
            Err(e) => error!("[Matchmaking] Failed to hydrate queue from SQLite: {e}"),
        }

        match sqlx::query_as::<_, (String, i64, String, bool, i64)>(
            "SELECT pubkey, game_id, opponent, is_white, matched_at FROM matchmaking_matches",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => {
                let count = rows.len();
                let mut matches = self
                    .matches
                    .lock()
                    .expect("Mutex lock should not be poisoned");
                for (pubkey, game_id, opponent, is_white, matched_at) in rows {
                    matches.insert(
                        pubkey,
                        MatchResult {
                            game_id: game_id as u64,
                            opponent,
                            is_white,
                            matched_at: matched_at as u64,
                        },
                    );
                }
                info!("[Matchmaking] Hydrated {count} pending match(es) from SQLite");
            }
            Err(e) => error!("[Matchmaking] Failed to hydrate matches from SQLite: {e}"),
        }
    }
}

// `Default` is for tests only — it points at devnet with a placeholder
// program id and a lazy, never-connected in-memory pool, which is fine in
// isolation but must never be used to build the real `AppState` (use
// `SharedMatchmakingState::new` there instead).
impl Default for SharedMatchmakingState {
    fn default() -> Self {
        let program_id = solana_sdk::pubkey::Pubkey::new_from_array([0u8; 32]);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .expect("lazy sqlite pool construction should not fail");
        Self::new(
            Arc::new(crate::signing::EloCache::new(
                "https://api.devnet.solana.com".to_string(),
                std::time::Duration::from_secs(300),
                program_id,
            )),
            pool,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn migrated_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../../../migrations/022_matchmaking_queue.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn state_with_pool(pool: sqlx::SqlitePool) -> SharedMatchmakingState {
        let program_id = solana_sdk::pubkey::Pubkey::new_from_array([0u8; 32]);
        SharedMatchmakingState::new(
            Arc::new(crate::signing::EloCache::new(
                "https://api.devnet.solana.com".to_string(),
                std::time::Duration::from_secs(300),
                program_id,
            )),
            pool,
        )
    }

    /// Simulates a backend restart: write a ticket + a match result via one
    /// `SharedMatchmakingState`, drop it, then hydrate a fresh instance
    /// sharing only the SQLite pool and confirm both come back. This is the
    /// property Phase 1 of the persistency plan exists to guarantee.
    #[tokio::test]
    async fn queue_and_match_survive_a_simulated_restart() {
        let pool = migrated_pool().await;

        {
            let state = state_with_pool(pool.clone());
            sqlx::query(
                "INSERT OR REPLACE INTO matchmaking_queue (pubkey, elo, joined_at) VALUES (?, ?, ?)",
            )
            .bind("player_a")
            .bind(1500_i64)
            .bind(1_000_i64)
            .execute(&state.pool)
            .await
            .unwrap();

            sqlx::query(
                "INSERT OR REPLACE INTO matchmaking_matches \
                 (pubkey, game_id, opponent, is_white, matched_at) VALUES (?, ?, ?, ?, ?)",
            )
            .bind("player_b")
            .bind(42_i64)
            .bind("player_c")
            .bind(true)
            .bind(1_000_i64)
            .execute(&state.pool)
            .await
            .unwrap();
            // `state` (and its in-memory queue/matches) is dropped here,
            // simulating the process exiting — only the SQLite rows remain.
        }

        let restarted = state_with_pool(pool);
        restarted.hydrate().await;

        let queue = restarted.queue.lock().unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].pubkey, "player_a");
        assert_eq!(queue[0].elo, 1500);

        let matches = restarted.matches.lock().unwrap();
        let m = matches
            .get("player_b")
            .expect("match should survive restart");
        assert_eq!(m.game_id, 42);
        assert_eq!(m.opponent, "player_c");
        assert!(m.is_white);
    }
}
