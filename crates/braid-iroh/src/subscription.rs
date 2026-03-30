use anyhow::Result;
use braid_core::Update;
use bytes::Bytes;
use iroh::EndpointId;
use iroh_gossip::api::{GossipReceiver, GossipSender};
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn url_to_topic(url: &str) -> TopicId {
    let hash = blake3::hash(url.as_bytes());
    TopicId::from_bytes(*hash.as_bytes())
}

pub struct SubscriptionManager {
    gossip: Gossip,
    senders: Arc<RwLock<HashMap<String, GossipSender>>>,
}

impl SubscriptionManager {
    pub fn new(gossip: Gossip) -> Self {
        Self {
            gossip,
            senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn normalize_url(url: &str) -> String {
        let url = url.trim();
        if url.starts_with('/') {
            url.to_string()
        } else {
            format!("/{}", url)
        }
    }

    pub async fn subscribe(
        &self,
        url: &str,
        bootstrap: Vec<EndpointId>,
    ) -> Result<(GossipSender, GossipReceiver)> {
        let normalized = Self::normalize_url(url);
        let topic_id = url_to_topic(&normalized);

        let topic = self.gossip.subscribe(topic_id, bootstrap).await?;
        let (sender, receiver) = topic.split();

        self.senders
            .write()
            .await
            .insert(normalized, sender.clone());

        Ok((sender, receiver))
    }

    pub async fn broadcast(&self, url: &str, update: &Update) -> Result<()> {
        let normalized = Self::normalize_url(url);
        let senders = self.senders.read().await;

        if let Some(sender) = senders.get(&normalized) {
            let bytes = serde_json::to_vec(update)
                .map_err(|e| anyhow::anyhow!("Failed to serialize update: {}", e))?;
            sender.broadcast(Bytes::from(bytes)).await?;
        } else {
            tracing::warn!("No sender for topic {}, message dropped", normalized);
        }

        Ok(())
    }

    pub async fn join_peers(&self, url: &str, peers: Vec<EndpointId>) -> Result<()> {
        let normalized = Self::normalize_url(url);
        let senders = self.senders.read().await;

        if let Some(sender) = senders.get(&normalized) {
            sender.join_peers(peers).await?;
        } else {
            tracing::warn!("No sender for topic {}, cannot join peers", normalized);
        }

        Ok(())
    }
}
