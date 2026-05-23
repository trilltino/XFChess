//! State management for P2P relay.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use super::types::ActiveGame;

/// Shared state for P2P relay
pub type P2PRelayState = Arc<RwLock<HashMap<String, ActiveGame>>>;

/// Creates the relay state and spawns cleanup task
pub fn create_relay_state() -> P2PRelayState {
    let state: P2PRelayState = Arc::new(RwLock::new(HashMap::new()));

    // Spawn cleanup task
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            cleanup_stale_games(&state_clone);
        }
    });

    state
}

/// Removes stale games (no activity for 5 minutes)
fn cleanup_stale_games(state: &P2PRelayState) {
    use super::types::GameStatus;

    let mut games = state.write().expect("P2P relay mutex should not be poisoned");
    let now = Utc::now();
    let stale_threshold = chrono::Duration::minutes(5);

    games.retain(|_, game| {
        let elapsed = now.signed_duration_since(game.last_activity);
        let is_stale = elapsed > stale_threshold;
        let is_finished = game.announcement.status == GameStatus::Finished;

        if is_stale || is_finished {
            tracing::info!(
                "Removing game {} (stale={}, finished={})",
                game.announcement.game_id, is_stale, is_finished
            );
        }

        // Keep only active games that still have a live host
        !is_stale && !is_finished
    });
}
