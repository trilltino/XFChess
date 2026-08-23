//! Tournament gossip service for real-time Swiss tournament updates.
//!
//! Manages gossip topics for tournaments using braid-iroh's gossip protocol,
//! and carries the Braid updates published by the [`ResourceHub`] out to peers.
//! Provides topic lifecycle management, update broadcasting, and bootstrap
//! peer discovery.
//!
//! # Late joiners
//!
//! This service carried a `tournament_gossip_log` SQLite table, plus
//! `persist_message` / `get_missed_messages` / `replay_missed_messages`, meant
//! to replay to a late peer whatever it had missed. None of it ever ran:
//! `server.rs` created the table at startup, and nothing anywhere called
//! `persist_message`, so the table stayed empty and the replay had nothing to
//! replay. It was scaffolding for a mechanism that was never finished.
//!
//! That is now the transport's job rather than this service's. Every fact is a
//! versioned update on a hub resource, so a late or reconnecting client gets
//! the current snapshot followed by the live tail by subscribing — over HTTP
//! `209` from [`crate::infrastructure::build_app_router`]'s `/braid` mount, or
//! over gossip from here. Nothing to persist, nothing to replay by hand.
//!
//! [`ResourceHub`]: xfchess_braid_server::ResourceHub

use anyhow::Result;
// Note: iroh crate not directly available, using String for node IDs
pub type EndpointId = String;
use rand::seq::IteratorRandom;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use xfchess_braid_server::resource::protocol::{encode_for_gossip, BraidUpdate};

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
}

impl TournamentGossipService {
    /// Create a new tournament gossip service
    pub fn new(store: TournamentStore, vps_node_id: Option<EndpointId>) -> Self {
        Self {
            store,
            tournament_topics: Arc::new(RwLock::new(HashMap::new())),
            vps_node_id,
        }
    }

    /// Broadcast one hub update to a tournament's peers.
    ///
    /// `path` is the resource the update belongs to (e.g.
    /// `tournament/42/standings`); it travels with the update, since a gossip
    /// receiver has no request line to read it from.
    ///
    /// Missing topic is not an error — a tournament nobody has subscribed to
    /// P2P still publishes over HTTP, and a subscriber that arrives later gets
    /// the state from the snapshot rather than from this broadcast.
    pub async fn broadcast_update(&self, tournament_id: u64, path: &str, update: &BraidUpdate) {
        let Some(sender) = self.get_topic(tournament_id).await else {
            return;
        };
        let Some(bytes) = encode_for_gossip(path, update) else {
            return;
        };
        if let Err(e) = sender.broadcast(bytes.into()).await {
            warn!(
                "[gossip] broadcast failed for {path} (v{}): {e}",
                update.version
            );
        } else {
            info!("[gossip] broadcast {path} v{}", update.version);
        }
    }

    /// Register a topic for a tournament with a live gossip sender
    pub async fn register_topic(&self, tournament_id: u64, sender: iroh_gossip::api::GossipSender) {
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
            info!(
                "[gossip] Ensured placeholder topic for tournament {}",
                tournament_id
            );
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
            .sample(&mut rand::rng(), 4);

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
        self.tournament_topics
            .read()
            .await
            .contains_key(&tournament_id)
    }

    /// Remove a topic for a tournament
    pub async fn remove_topic(&self, tournament_id: u64) {
        self.tournament_topics.write().await.remove(&tournament_id);
        info!("[gossip] Removed topic for tournament {}", tournament_id);
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
    use std::sync::Mutex;
    use xfchess_braid_server::{bridge, ResourceHub};

    /// The full publish path, end to end: a Swiss write lands on a hub
    /// resource, the sink encodes that update for gossip, and a client decodes
    /// it back into the event it was before this went through Braid.
    ///
    /// This is the seam the refactor introduced — server-side `BraidUpdate`,
    /// wire `Update`, client-side `SwissMessage` — and the one place where a
    /// path typo or a lost content-type silently degrades into "peers receive
    /// nothing" rather than a compile error.
    #[test]
    fn a_hub_write_reaches_a_client_as_the_same_event() {
        let hub = ResourceHub::new();
        let captured: Arc<Mutex<Vec<(String, BraidUpdate)>>> = Arc::new(Mutex::new(Vec::new()));

        let sink_captured = captured.clone();
        hub.set_gossip_sink(Arc::new(move |path: &str, update: &BraidUpdate| {
            sink_captured
                .lock()
                .expect("sink mutex")
                .push((path.to_string(), update.clone()));
        }));

        bridge::push_standings(
            &hub,
            7,
            serde_json::json!([{ "player_id": "alice", "score": 1.5, "rank": 1 }]),
        );

        let events = captured.lock().expect("sink mutex");
        let (path, update) = events
            .iter()
            .find(|(p, _)| p.ends_with("/standings"))
            .expect("a standings write must reach the gossip sink");
        assert_eq!(path, "tournament/7/standings");

        let bytes = encode_for_gossip(path, update).expect("update should encode");
        let wire: braid_chess::braid_http::types::Update =
            serde_json::from_slice(&bytes).expect("encoded update should be valid JSON");

        assert_eq!(
            braid_chess::SwissMessage::from_update(&wire),
            Some(braid_chess::SwissMessage::StandingsUpdated {
                tournament_id: 7,
                standings: vec![braid_chess::SwissStandingsEntry {
                    player_id: "alice".into(),
                    score: 1.5,
                    rank: 1,
                }],
            })
        );
    }

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

    /// A tournament nobody has joined P2P has no gossip topic. Broadcasting
    /// into that must be a quiet no-op, not an error: the same update still
    /// reached every HTTP `209` subscriber through the hub.
    #[tokio::test]
    async fn broadcast_without_a_topic_is_a_no_op() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");
        let tournament_store = TournamentStore::new(pool).await;
        let gossip = TournamentGossipService::new(tournament_store, None);

        let update = BraidUpdate::snapshot(1, serde_json::json!([]));
        gossip
            .broadcast_update(42, "tournament/42/standings", &update)
            .await;

        assert!(!gossip.has_topic(42).await);
    }
}
