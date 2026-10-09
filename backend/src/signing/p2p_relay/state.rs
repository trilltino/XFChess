use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use super::types::{ActiveGame, LOBBY_TTL_SECS};

pub type P2PRelayState = Arc<RwLock<HashMap<String, ActiveGame>>>;

/// Write-through persistence for relay rooms (migration 033).
///
/// The in-memory map stays the serving copy; every mutation is mirrored to
/// SQLite so a backend restart can hydrate open lobbies, pending JOIN_ACK
/// handshakes and undelivered mailbox messages instead of silently dropping
/// them. Persistence failures are logged, not surfaced: the relay is advisory
/// pre-game signalling, and move/result authority lives elsewhere.
#[derive(Clone)]
pub struct RelayStore {
    pool: sqlx::SqlitePool,
}

impl RelayStore {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save(&self, game: &ActiveGame) {
        let json = match serde_json::to_string(game) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!("[p2p-relay] could not serialize room: {e}");
                return;
            }
        };
        if let Err(e) = sqlx::query(
            "INSERT INTO p2p_relay_rooms (game_id, room_json, last_activity) VALUES (?, ?, ?) \
             ON CONFLICT(game_id) DO UPDATE SET room_json = excluded.room_json, \
             last_activity = excluded.last_activity",
        )
        .bind(&game.announcement.game_id)
        .bind(json)
        .bind(game.last_activity.timestamp())
        .execute(&self.pool)
        .await
        {
            tracing::warn!(
                "[p2p-relay] could not persist room {}: {e}",
                game.announcement.game_id
            );
        }
    }

    pub async fn delete(&self, game_ids: &[String]) {
        for game_id in game_ids {
            if let Err(e) = sqlx::query("DELETE FROM p2p_relay_rooms WHERE game_id = ?")
                .bind(game_id)
                .execute(&self.pool)
                .await
            {
                tracing::warn!("[p2p-relay] could not delete room {game_id}: {e}");
            }
        }
    }

    /// Load persisted rooms into `state`. Rooms already past the TTL are
    /// dropped (and deleted) rather than resurrected; a room already in
    /// memory is never overwritten by its older persisted copy.
    pub async fn hydrate(&self, state: &P2PRelayState) -> Result<usize, sqlx::Error> {
        let rows =
            sqlx::query_as::<_, (String, String)>("SELECT game_id, room_json FROM p2p_relay_rooms")
                .fetch_all(&self.pool)
                .await?;
        let now = Utc::now();
        let mut expired = Vec::new();
        let mut restored = 0;
        {
            let mut games = state
                .write()
                .expect("P2P relay mutex should not be poisoned");
            for (game_id, json) in rows {
                match serde_json::from_str::<ActiveGame>(&json) {
                    Ok(game) if !is_expired(&game, now) => {
                        if !games.contains_key(&game_id) {
                            games.insert(game_id, game);
                            restored += 1;
                        }
                    }
                    Ok(_) => expired.push(game_id),
                    Err(e) => {
                        tracing::warn!("[p2p-relay] dropping unreadable room {game_id}: {e}");
                        expired.push(game_id);
                    }
                }
            }
        }
        self.delete(&expired).await;
        Ok(restored)
    }
}

fn is_expired(game: &ActiveGame, now: chrono::DateTime<Utc>) -> bool {
    use super::types::GameStatus;
    now.signed_duration_since(game.last_activity) > chrono::Duration::seconds(LOBBY_TTL_SECS)
        || game.announcement.status == GameStatus::Finished
}

pub fn create_relay_state(store: Option<RelayStore>) -> P2PRelayState {
    let state: P2PRelayState = Arc::new(RwLock::new(HashMap::new()));

    // Spawn cleanup task. Runs more often than `LOBBY_TTL_SECS` so a dead
    // lobby is never visibly stale for much longer than the TTL itself.
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            let removed = cleanup_stale_games(&state_clone);
            if let Some(store) = &store {
                store.delete(&removed).await;
            }
        }
    });

    state
}

fn cleanup_stale_games(state: &P2PRelayState) -> Vec<String> {
    let mut games = state
        .write()
        .expect("P2P relay mutex should not be poisoned");
    let now = Utc::now();
    let mut removed = Vec::new();

    games.retain(|game_id, game| {
        if is_expired(game, now) {
            tracing::info!(
                "Removing game {} (stale or finished)",
                game.announcement.game_id
            );
            removed.push(game_id.clone());
            false
        } else {
            true
        }
    });
    removed
}
