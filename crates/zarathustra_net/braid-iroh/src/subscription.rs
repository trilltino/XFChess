use braid_http::Update;
use bytes::Bytes;
use iroh::EndpointId;
use iroh_gossip::api::{GossipReceiver, GossipSender, GossipTopic};
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

struct TopicEntry {
    sender: GossipSender,
    last_active: Instant,
}

const TOPIC_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const TOPIC_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub struct SubscriptionManager {
    gossip: Gossip,
    topics: Arc<Mutex<HashMap<String, TopicEntry>>>,
    sweep_task: tokio::task::JoinHandle<()>,
}

impl SubscriptionManager {
    pub fn new(gossip: Gossip) -> Self {
        let topics: Arc<Mutex<HashMap<String, TopicEntry>>> = Arc::new(Mutex::new(HashMap::new()));

        let sweep_topics = topics.clone();
        let sweep_task = tokio::spawn(async move {
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

        Self {
            gossip,
            topics,
            sweep_task,
        }
    }

    pub fn normalize_url(url: &str) -> String {
        format!("/{}", url.trim_matches('/'))
    }
    pub fn topic_for_url(url: &str) -> TopicId {
        let normalized = Self::normalize_url(url);
        let hash = blake3::hash(normalized.as_bytes());
        TopicId::from_bytes(*hash.as_bytes())
    }

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

    pub async fn broadcast(&self, url: &str, update: &Update) -> anyhow::Result<()> {
        let normalized = Self::normalize_url(url);
        let bytes = serde_json::to_vec(update)?;
        self.broadcast_raw(&normalized, Bytes::from(bytes)).await
    }

    pub async fn broadcast_raw(&self, url: &str, data: Bytes) -> anyhow::Result<()> {
        let normalized = Self::normalize_url(url);
        let sender = {
            let mut topics = self.topics.lock().await;
            topics.get_mut(&normalized).map(|entry| {
                entry.last_active = Instant::now();
                entry.sender.clone()
            })
        };
        let sender = match sender {
            Some(sender) => sender,
            None => {
                let topic = self
                    .gossip
                    .subscribe(Self::topic_for_url(&normalized), vec![])
                    .await?;
                let (candidate, _receiver) = topic.split();
                let mut topics = self.topics.lock().await;
                let entry = topics.entry(normalized).or_insert_with(|| TopicEntry {
                    sender: candidate,
                    last_active: Instant::now(),
                });
                entry.last_active = Instant::now();
                entry.sender.clone()
            }
        };
        sender.broadcast(data).await?;
        Ok(())
    }
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

impl Drop for SubscriptionManager {
    fn drop(&mut self) {
        self.sweep_task.abort();
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

        assert_eq!(topic_bytes.len(), 32);
    }

    #[test]
    fn test_topic_derivation_long_url() {
        let long_url = "/a/very/long/url/path".repeat(100);
        let topic = SubscriptionManager::topic_for_url(&long_url);
        let topic_bytes: &[u8; 32] = topic.as_ref();

        assert_eq!(topic_bytes.len(), 32);
    }

    #[test]
    fn test_topic_derivation_special_chars() {
        let url = "/resource/with spaces/and/special/chars/!@#$%";
        let topic1 = SubscriptionManager::topic_for_url(url);
        let topic2 = SubscriptionManager::topic_for_url(url);

        assert_eq!(topic1, topic2);
    }
}
