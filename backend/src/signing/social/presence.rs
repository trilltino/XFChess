//! In-memory presence store.  Tracks which node IDs are online / in-game.
//! Entries expire after 5 minutes of silence (same TTL as P2P lobbies).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;

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
    /// game_id when status == InGame
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
            info!("[Presence] {} is {:?}", p.display_name, p.status);
            map.insert(p.node_id.clone(), p);
        }
    }

    pub fn get_all_online(&self) -> Vec<Presence> {
        self.inner
            .read()
            .map(|m| {
                let cutoff = Utc::now() - chrono::Duration::minutes(5);
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

    /// Mark a node as offline (called on disconnect / heartbeat timeout).
    pub fn set_offline(&self, node_id: &str) {
        if let Ok(mut map) = self.inner.write() {
            if let Some(p) = map.get_mut(node_id) {
                p.status = PresenceStatus::Offline;
                p.updated_at = Utc::now();
            }
        }
    }

    /// Sweep stale entries (>10 min old) — call from a background task.
    pub fn sweep_stale(&self) {
        if let Ok(mut map) = self.inner.write() {
            let cutoff = Utc::now() - chrono::Duration::minutes(10);
            map.retain(|_, p| p.updated_at > cutoff);
        }
    }
}
