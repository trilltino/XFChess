//! Gossip-backed Braid subscriptions.
//!
//! Maps resource URLs to iroh-gossip topics. When a peer PUTs an update,
//! it gets broadcast to everyone subscribed to that URL's topic.
//! This replaces Braid's traditional long-lived HTTP subscription responses
//! with fully decentralized gossip.

use braid_core::Update;
use bytes::Bytes;
use iroh::EndpointId;
use iroh_gossip::api::{GossipReceiver, GossipSender, GossipTopic};
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// A topic we've joined, plus when it was last used — lets the sweep in
/// [`SubscriptionManager::new`] evict topics nobody has broadcast to or
/// subscribed to in a long time, instead of holding every gossip session
/// this node has ever touched for its whole lifetime.
struct TopicEntry {
    sender: GossipSender,
    last_active: Instant,
}

/// Topics untouched for this long are dropped by the periodic sweep.
const TOPIC_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const TOPIC_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Manages active gossip subscriptions keyed by resource URL.
///
/// Each URL gets a deterministic `TopicId` via blake3 hash, so any peer
/// that knows the URL can join the correct topic without coordination.
pub struct SubscriptionManager {
    gossip: Gossip,
    /// Active topics: URL → sender handle + last-used time
    topics: Arc<Mutex<HashMap<String, TopicEntry>>>,
}

impl SubscriptionManager {
    /// Wrap an existing gossip instance. Spawns a background sweep that
    /// drops topics idle for longer than [`TOPIC_IDLE_TIMEOUT`] — otherwise
    /// a long-running relay node accumulates one live gossip session per
    /// resource URL it has ever seen, forever.
    pub fn new(gossip: Gossip) -> Self {
        let topics: Arc<Mutex<HashMap<String, TopicEntry>>> = Arc::new(Mutex::new(HashMap::new()));

        let sweep_topics = topics.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TOPIC_SWEEP_INTERVAL);
            loop {
                interval.tick().await;
                let mut topics = sweep_topics.lock().await;
                let before = topics.len();
                topics.retain(|_, entry| entry.last_active.elapsed() < TOPIC_IDLE_TIMEOUT);
                let removed = before - topics.len();
                if removed > 0 {
                    tracing::debug!(
                        removed,
                        remaining = topics.len(),
                        "swept idle gossip topics"
                    );
                }
            }
        });

        Self { gossip, topics }
    }

    /// Normalize a URL for consistent topic lookup.
    /// Ensures the URL starts with / and has no trailing /.
    pub fn normalize_url(url: &str) -> String {
        if url.starts_with('/') {
            url.trim_end_matches('/').to_string()
        } else {
            format!("/{}", url.trim_end_matches('/'))
        }
    }

    /// Derive a deterministic TopicId from a resource URL.
    /// Any peer hashing the same URL gets the same topic.
    /// Normalizes the URL to ensure consistency.
    pub fn topic_for_url(url: &str) -> TopicId {
        let normalized = Self::normalize_url(url);
        let hash = blake3::hash(normalized.as_bytes());
        TopicId::from_bytes(*hash.as_bytes())
    }

    /// Subscribe to a resource URL. Joins the gossip topic and returns
    /// a receiver stream of incoming gossip events.
    ///
    /// `bootstrap` should contain at least one known peer so the gossip
    /// protocol can form the initial overlay.
    pub async fn subscribe(
        &self,
        url: &str,
        bootstrap: Vec<EndpointId>,
    ) -> anyhow::Result<(GossipSender, GossipReceiver)> {
        let normalized = Self::normalize_url(url);
        let topic_id = Self::topic_for_url(&normalized);
        let topic: GossipTopic = self.gossip.subscribe(topic_id, bootstrap).await?;
        let (sender, receiver) = topic.split();

        // Stash the sender so we can broadcast later
        self.topics.lock().await.insert(
            normalized,
            TopicEntry {
                sender: sender.clone(),
                last_active: Instant::now(),
            },
        );

        Ok((sender, receiver))
    }

    /// Broadcast a Braid Update to all peers on a resource's gossip topic.
    /// Serializes the update to JSON bytes before sending.
    pub async fn broadcast(&self, url: &str, update: &Update) -> anyhow::Result<()> {
        let normalized = Self::normalize_url(url);
        let bytes = serde_json::to_vec(update)?;
        self.broadcast_raw(&normalized, Bytes::from(bytes)).await
    }

    /// Broadcast raw bytes to all peers on a resource's gossip topic.
    /// This allows sending wrapped messages with metadata.
    ///
    /// Only holds the `topics` lock long enough to look up (or create) and
    /// clone the sender — `GossipSender` is a cheap `Clone` over an mpsc
    /// handle, so the actual `.broadcast().await` (which can block for an
    /// unbounded time if the gossip actor is backed up) runs lock-free.
    /// Otherwise a single slow topic would serialize broadcasts to every
    /// other, unrelated topic behind this one lock.
    pub async fn broadcast_raw(&self, url: &str, data: Bytes) -> anyhow::Result<()> {
        let normalized = Self::normalize_url(url);
        let sender = {
            let mut topics = self.topics.lock().await;
            tracing::debug!(
                url,
                normalized = normalized.as_str(),
                topics_count = topics.len(),
                "broadcast_raw"
            );

            // If we don't have a sender for this topic, join it (with no bootstrap peers)
            // This allows us to publish to a topic we haven't explicitly subscribed to
            if !topics.contains_key(&normalized) {
                tracing::debug!(normalized = normalized.as_str(), "creating new topic");
                let topic_id = Self::topic_for_url(&normalized);
                // Join with empty bootstrap peers since we are likely the publisher/origin
                let topic: GossipTopic = self.gossip.subscribe(topic_id, vec![]).await?;
                let (sender, _receiver) = topic.split();

                // We discard the receiver because we don't necessarily want to listen to our own updates
                // (or maybe we do? but for now just enable publishing)
                topics.insert(
                    normalized.clone(),
                    TopicEntry {
                        sender,
                        last_active: Instant::now(),
                    },
                );
            } else if let Some(entry) = topics.get_mut(&normalized) {
                entry.last_active = Instant::now();
            }

            topics.get(&normalized).map(|entry| entry.sender.clone())
        };

        if let Some(sender) = sender {
            tracing::debug!(
                normalized = normalized.as_str(),
                bytes = data.len(),
                "sending broadcast"
            );
            sender.broadcast(data).await?;
            tracing::debug!(normalized = normalized.as_str(), "broadcast complete");
        }
        Ok(())
    }

    /// Access the underlying gossip instance (e.g. for shutdown).
    #[allow(dead_code)]
    pub fn gossip(&self) -> &Gossip {
        &self.gossip
    }

    /// Join additional peers to an existing topic.
    /// This is useful for connecting to a peer after initial subscription.
    pub async fn join_peers(&self, url: &str, peers: Vec<EndpointId>) -> anyhow::Result<()> {
        let normalized = Self::normalize_url(url);
        let sender = {
            let mut topics = self.topics.lock().await;
            match topics.get_mut(&normalized) {
                Some(entry) => {
                    entry.last_active = Instant::now();
                    Some(entry.sender.clone())
                }
                None => None,
            }
        };
        match sender {
            Some(sender) => {
                sender.join_peers(peers).await?;
                Ok(())
            }
            None => anyhow::bail!("Topic not found: {}", normalized),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_derivation_deterministic() {
        let url = "/test/resource/123";
        let topic1 = SubscriptionManager::topic_for_url(url);
        let topic2 = SubscriptionManager::topic_for_url(url);

        assert_eq!(topic1, topic2);
    }

    #[test]
    fn test_topic_derivation_unique_per_url() {
        let topic1 = SubscriptionManager::topic_for_url("/resource/a");
        let topic2 = SubscriptionManager::topic_for_url("/resource/b");

        assert_ne!(topic1, topic2);
    }

    #[test]
    fn test_topic_derivation_empty_url() {
        let topic = SubscriptionManager::topic_for_url("");
        let topic_bytes: &[u8; 32] = topic.as_ref();

        // Should produce a valid 32-byte hash (not panic)
        assert_eq!(topic_bytes.len(), 32);
    }

    #[test]
    fn test_topic_derivation_long_url() {
        let long_url = "/a/very/long/url/path".repeat(100);
        let topic = SubscriptionManager::topic_for_url(&long_url);
        let topic_bytes: &[u8; 32] = topic.as_ref();

        // Should still produce a valid 32-byte hash
        assert_eq!(topic_bytes.len(), 32);
    }

    #[test]
    fn test_topic_derivation_special_chars() {
        let url = "/resource/with spaces/and/special/chars/!@#$%";
        let topic1 = SubscriptionManager::topic_for_url(url);
        let topic2 = SubscriptionManager::topic_for_url(url);

        // Should be deterministic even with special characters
        assert_eq!(topic1, topic2);
    }
}
