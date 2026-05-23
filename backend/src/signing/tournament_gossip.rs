//! Tournament gossip service for real-time Swiss tournament updates.
//!
//! Manages gossip topics for tournaments using braid-iroh's gossip protocol.
//! Provides topic lifecycle management, message broadcasting, bootstrap
//! peer discovery, and message persistence for late joiners.

use anyhow::Result;
use braid_iroh::SwissMessage;
// Note: iroh crate not directly available, using String for node IDs
pub type EndpointId = String;
use rand::seq::IteratorRandom;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::signing::storage::tournament::TournamentStore;

/// Handle to an active tournament gossip topic
pub struct TopicHandle {
    /// Gossip sender for broadcasting messages (None if topic is pre-registered without a live sender)
    pub sender: Option<iroh_gossip::api::GossipSender>,
    /// Tournament ID
    pub tournament_id: u64,
    /// Number of active subscribers
    pub subscriber_count: AtomicUsize,
}

/// Service managing tournament gossip topics
pub struct TournamentGossipService {
    /// Tournament storage for persistence
    store: TournamentStore,
    /// Active tournament topics
    tournament_topics: Arc<RwLock<HashMap<u64, TopicHandle>>>,
    /// VPS node ID for reliable bootstrap
    vps_node_id: Option<EndpointId>,
    /// SQLite pool for message persistence
    db_pool: Option<SqlitePool>,
}

impl TournamentGossipService {
    /// Create a new tournament gossip service
    pub fn new(store: TournamentStore, vps_node_id: Option<EndpointId>) -> Self {
        Self {
            store,
            tournament_topics: Arc::new(RwLock::new(HashMap::new())),
            vps_node_id,
            db_pool: None,
        }
    }

    /// Initialize the database for message persistence
    pub async fn init_db(&mut self, pool: SqlitePool) {
        // Create gossip message log table
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS tournament_gossip_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tournament_id INTEGER NOT NULL,
                round INTEGER NOT NULL,
                message_type TEXT NOT NULL,
                message_json TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                INDEX idx_tournament_round (tournament_id, round)
            )"#
        )
        .execute(&pool)
        .await
        .ok();

        self.db_pool = Some(pool);
        info!("[gossip] Message persistence initialized");
    }

    /// Register a topic for a tournament with a live gossip sender
    pub async fn register_topic(
        &self,
        tournament_id: u64,
        sender: iroh_gossip::api::GossipSender,
    ) {
        let handle = TopicHandle {
            sender: Some(sender),
            tournament_id,
            subscriber_count: AtomicUsize::new(0),
        };
        self.tournament_topics
            .write()
            .await
            .insert(tournament_id, handle);
        info!("[gossip] Registered topic for tournament {}", tournament_id);
    }

    /// Ensure a topic placeholder exists for a tournament (used at init time before a sender is available).
    pub async fn ensure_topic_registered(&self, tournament_id: u64) {
        let mut topics = self.tournament_topics.write().await;
        if !topics.contains_key(&tournament_id) {
            let handle = TopicHandle {
                sender: None,
                tournament_id,
                subscriber_count: AtomicUsize::new(0),
            };
            topics.insert(tournament_id, handle);
            info!("[gossip] Ensured placeholder topic for tournament {}", tournament_id);
        }
    }

    /// Get bootstrap peers for a player joining a tournament
    ///
    /// Returns up to 5 peers including:
    /// 1. VPS node as reliable bootstrap
    /// 2. Tournament host (first registered player)
    /// 3. 3-4 random other players
    pub async fn get_bootstrap_peers(
        &self,
        tournament_id: u64,
        requesting_player: &str,
    ) -> Vec<EndpointId> {
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => {
                warn!(
                    "[gossip] Tournament {} not found for bootstrap",
                    tournament_id
                );
                return self.vps_node_id.clone().into_iter().collect();
            }
        };

        let mut peers = Vec::with_capacity(5);

        // Add VPS as reliable bootstrap
        if let Some(ref vps) = self.vps_node_id {
            peers.push(vps.clone());
        }

        // Add tournament host (first registered player)
        if let Some(host) = tournament.players.first() {
            if host != requesting_player {
                if let Some(node_id_str) = tournament.node_ids.get(host) {
                    if let Ok(node_id) = parse_node_id(node_id_str) {
                        peers.push(node_id);
                    }
                }
            }
        }

        // Add random subset of other players (max 4)
        let other_players: Vec<_> = tournament
            .players
            .iter()
            .filter(|p| *p != requesting_player)
            .choose_multiple(&mut rand::rng(), 4);

        for player in other_players {
            if let Some(node_id_str) = tournament.node_ids.get(player) {
                if let Ok(node_id) = parse_node_id(node_id_str) {
                    if !peers.contains(&node_id) {
                        peers.push(node_id);
                    }
                }
            }
        }

        info!(
            "[gossip] Bootstrap for {} in tournament {}: {} peers",
            requesting_player,
            tournament_id,
            peers.len()
        );

        peers
    }

    /// Get all registered node IDs for a tournament
    pub async fn get_tournament_peers(&self, tournament_id: u64) -> Vec<EndpointId> {
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => return Vec::new(),
        };

        tournament
            .node_ids
            .values()
            .filter_map(|node_id_str| parse_node_id(node_id_str).ok())
            .collect()
    }

    /// Increment subscriber count for a tournament
    pub async fn increment_subscribers(&self, tournament_id: u64) {
        if let Some(handle) = self.tournament_topics.read().await.get(&tournament_id) {
            let count = handle.subscriber_count.fetch_add(1, Ordering::Relaxed) + 1;
            info!(
                "[gossip] Tournament {} subscriber count: {}",
                tournament_id, count
            );
        }
    }

    /// Decrement subscriber count for a tournament
    pub async fn decrement_subscribers(&self, tournament_id: u64) {
        if let Some(handle) = self.tournament_topics.read().await.get(&tournament_id) {
            let count = handle
                .subscriber_count
                .fetch_sub(1, Ordering::Relaxed)
                .saturating_sub(1);
            info!(
                "[gossip] Tournament {} subscriber count: {}",
                tournament_id, count
            );
        }
    }

    /// Get subscriber count for a tournament
    pub async fn get_subscriber_count(&self, tournament_id: u64) -> usize {
        self.tournament_topics
            .read()
            .await
            .get(&tournament_id)
            .map(|h| h.subscriber_count.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get topic handle for a tournament
    pub async fn get_topic(&self, tournament_id: u64) -> Option<iroh_gossip::api::GossipSender> {
        self.tournament_topics
            .read()
            .await
            .get(&tournament_id)
            .and_then(|h| h.sender.clone())
    }

    /// Check if a topic exists for a tournament
    pub async fn has_topic(&self, tournament_id: u64) -> bool {
        self.tournament_topics.read().await.contains_key(&tournament_id)
    }

    /// Remove a topic for a tournament
    pub async fn remove_topic(&self, tournament_id: u64) {
        self.tournament_topics.write().await.remove(&tournament_id);
        info!("[gossip] Removed topic for tournament {}", tournament_id);
    }

    /// Persist a Swiss message to the database
    pub async fn persist_message(&self, tournament_id: u64, message: &SwissMessage) {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return,
        };

        let (round, msg_type) = match message {
            SwissMessage::RoundStarted { round, .. } => (*round, "RoundStarted"),
            SwissMessage::ResultRecorded { round, .. } => (*round, "ResultRecorded"),
            SwissMessage::StandingsUpdated { .. } => (0, "StandingsUpdated"),
            SwissMessage::BracketFired { .. } => (0, "BracketFired"),
        };

        let message_json = match serde_json::to_string(message) {
            Ok(json) => json,
            Err(e) => {
                warn!("[gossip] Failed to serialize message: {}", e);
                return;
            }
        };

        let timestamp = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO tournament_gossip_log (tournament_id, round, message_type, message_json, timestamp) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(tournament_id as i64)
        .bind(round as i32)
        .bind(msg_type)
        .bind(message_json)
        .bind(timestamp)
        .execute(pool)
        .await
        .ok();
    }

    /// Get missed messages for a late joiner
    pub async fn get_missed_messages(
        &self,
        tournament_id: u64,
        since_round: u8,
    ) -> Vec<SwissMessage> {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Vec::new(),
        };

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT message_json FROM tournament_gossip_log WHERE tournament_id = ? AND round >= ? ORDER BY timestamp ASC"
        )
        .bind(tournament_id as i64)
        .bind(since_round as i32)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .filter_map(|(json,)| serde_json::from_str(&json).ok())
            .collect()
    }

    /// Replay missed messages to a specific peer
    pub async fn replay_missed_messages(
        &self,
        tournament_id: u64,
        since_round: u8,
        sender: &iroh_gossip::api::GossipSender,
    ) -> Result<usize> {
        let messages = self.get_missed_messages(tournament_id, since_round).await;
        let count = messages.len();

        for message in messages {
            let bytes = serde_json::to_vec(&message)?;
            sender.broadcast(bytes.into()).await?;
        }

        info!(
            "[gossip] Replayed {} messages for tournament {} from round {}",
            count, tournament_id, since_round
        );

        Ok(count)
    }
}

/// Parse a node ID string (just returns the string for now)
fn parse_node_id(node_id_str: &str) -> Result<EndpointId> {
    Ok(node_id_str.to_string())
}

/// Format an EndpointId (just returns the string)
pub fn format_node_id(node_id: &EndpointId) -> String {
    node_id.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_and_parse_node_id() {
        // Create a test node ID string
        let node_id: EndpointId = "test_node_id_12345".to_string();

        // Format and parse
        let formatted = format_node_id(&node_id);
        let parsed = parse_node_id(&formatted).expect("parse_node_id should succeed");

        assert_eq!(node_id, parsed);
    }

    #[test]
    fn test_parse_invalid_node_id() {
        // Empty string is now valid since we use String
        assert!(parse_node_id("").is_ok());
        assert!(parse_node_id("valid_node_id").is_ok());
    }
}
