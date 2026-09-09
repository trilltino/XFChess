use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;

pub const ONLINE_FRESHNESS_SECS: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    Online,
    InGame,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub node_id: String,
    pub pubkey: Option<String>,
    pub display_name: String,
    pub status: PresenceStatus,
    pub game_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct PresenceStore {
    inner: Arc<RwLock<HashMap<String, Presence>>>,
}

impl PresenceStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, p: Presence) {
        if let Ok(mut map) = self.inner.write() {
            let changed = map
                .get(&p.node_id)
                .map(|prev| prev.status != p.status)
                .unwrap_or(true);
            if changed {
                info!("[Presence] {} is {:?}", p.display_name, p.status);
            }
            map.insert(p.node_id.clone(), p);
        }
    }

    pub fn get_all_online(&self) -> Vec<Presence> {
        self.inner
            .read()
            .map(|m| {
                let cutoff = Utc::now() - chrono::Duration::seconds(ONLINE_FRESHNESS_SECS);
                m.values()
                    .filter(|p| p.updated_at > cutoff && p.status != PresenceStatus::Offline)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get(&self, node_id: &str) -> Option<Presence> {
        self.inner.read().ok()?.get(node_id).cloned()
    }

    pub fn set_offline(&self, node_id: &str) {
        if let Ok(mut map) = self.inner.write() {
            if let Some(p) = map.get_mut(node_id) {
                p.status = PresenceStatus::Offline;
                p.updated_at = Utc::now();
            }
        }
    }

    pub fn sweep_stale(&self) {
        if let Ok(mut map) = self.inner.write() {
            let cutoff = Utc::now() - chrono::Duration::minutes(10);
            map.retain(|_, p| p.updated_at > cutoff);
        }
    }

    pub fn count_in_game(&self) -> usize {
        self.get_all_online()
            .iter()
            .filter(|p| p.status == PresenceStatus::InGame)
            .count()
    }

    pub fn count_games_in_progress(&self) -> usize {
        let ids: std::collections::HashSet<String> = self
            .get_all_online()
            .into_iter()
            .filter(|p| p.status == PresenceStatus::InGame)
            .filter_map(|p| p.game_id)
            .collect();
        ids.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(node_id: &str, status: PresenceStatus, game_id: Option<&str>) -> Presence {
        Presence {
            node_id: node_id.to_string(),
            pubkey: None,
            display_name: node_id.to_string(),
            status,
            game_id: game_id.map(str::to_string),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn menu_only_players_dont_count_as_in_game() {
        let store = PresenceStore::new();
        store.upsert(presence("a", PresenceStatus::Online, None));
        store.upsert(presence("b", PresenceStatus::Online, None));
        assert_eq!(store.count_in_game(), 0);
        assert_eq!(store.count_games_in_progress(), 0);
    }

    #[test]
    fn bot_game_counts_as_online_but_not_a_game_in_progress() {
        let store = PresenceStore::new();
        // Solo bot/local game: in-game, but no game_id (no real opponent).
        store.upsert(presence("solo", PresenceStatus::InGame, None));
        assert_eq!(store.count_in_game(), 1);
        assert_eq!(store.count_games_in_progress(), 0);
    }

    #[test]
    fn two_players_in_same_multiplayer_game_count_as_one_game() {
        let store = PresenceStore::new();
        store.upsert(presence("white", PresenceStatus::InGame, Some("game-1")));
        store.upsert(presence("black", PresenceStatus::InGame, Some("game-1")));
        assert_eq!(store.count_in_game(), 2);
        assert_eq!(store.count_games_in_progress(), 1);
    }

    #[test]
    fn distinct_game_ids_count_as_separate_games() {
        let store = PresenceStore::new();
        store.upsert(presence("a1", PresenceStatus::InGame, Some("game-1")));
        store.upsert(presence("a2", PresenceStatus::InGame, Some("game-1")));
        store.upsert(presence("b1", PresenceStatus::InGame, Some("game-2")));
        store.upsert(presence("b2", PresenceStatus::InGame, Some("game-2")));
        assert_eq!(store.count_in_game(), 4);
        assert_eq!(store.count_games_in_progress(), 2);
    }

    #[test]
    fn offline_players_are_excluded_entirely() {
        let store = PresenceStore::new();
        store.upsert(presence("gone", PresenceStatus::Offline, None));
        assert_eq!(store.count_in_game(), 0);
        assert_eq!(store.count_games_in_progress(), 0);
    }
}
