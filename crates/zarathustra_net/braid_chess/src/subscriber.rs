use crate::error::BraidChessError;
use crate::message::ChessMessage;
use crate::resource::ChessResource;
use async_channel::Receiver;
use braid_http::client::{Acknowledger, Subscription};
use braid_http::types::{BraidRequest, Update, Version};
use braid_http::BraidClient;

use tokio::task::JoinHandle;
use tracing::{debug, warn};

pub struct ChessSubscriber {
    base_url: String,
    game_id: String,
    client: BraidClient,
}

pub struct DeliveryReceipt {
    versions: Vec<Version>,
    acknowledger: Option<Acknowledger>,
}

impl DeliveryReceipt {
    pub fn versions(&self) -> &[Version] {
        &self.versions
    }

    pub fn acknowledge(mut self) -> braid_http::Result<()> {
        self.acknowledger
            .take()
            .expect("receipt is consumed once")
            .acknowledge(&self.versions)
    }
}

impl Drop for DeliveryReceipt {
    fn drop(&mut self) {
        if let Some(acknowledger) = self.acknowledger.take() {
            let _ = acknowledger.reject(&self.versions);
        }
    }
}

pub struct ChessDelivery {
    pub message: ChessMessage,
    pub receipt: DeliveryReceipt,
}

impl ChessSubscriber {
    pub async fn subscribe_acknowledged(
        &self,
        resource: ChessResource,
        parents: Vec<Version>,
    ) -> Result<(Receiver<ChessDelivery>, JoinHandle<()>), BraidChessError> {
        let request = BraidRequest::new()
            .subscribe()
            .with_parents(parents)
            .with_manual_ack()
            .with_heartbeat(20)
            .with_retry(braid_http::client::RetryConfig::default().with_max_retries(5));
        let mut subscription = self
            .client
            .subscribe(&resource.to_url(&self.base_url), request)
            .await
            .map_err(|e| match e {
                braid_http::BraidError::InvalidSubscriptionStatus(status) => {
                    BraidChessError::HttpStatus(status)
                }
                error => BraidChessError::Http(error.to_string()),
            })?;
        let acknowledger = subscription
            .acknowledger()
            .expect("manual acknowledgment configured");
        let (tx, rx) = async_channel::bounded(1);
        let handle = tokio::spawn(async move {
            loop {
                let update = tokio::select! { _ = tx.closed() => break, update = subscription.next() => update };
                let Some(Ok(update)) = update else {
                    break;
                };
                let Some(message) = decode_update(&update) else {
                    let _ = acknowledger.reject(&update.version);
                    break;
                };
                let delivery = ChessDelivery {
                    message,
                    receipt: DeliveryReceipt {
                        versions: update.version,
                        acknowledger: Some(acknowledger.clone()),
                    },
                };
                if tx.send(delivery).await.is_err() {
                    break;
                }
            }
        });
        Ok((rx, handle))
    }
    pub fn new(
        base_url: impl Into<String>,
        game_id: impl Into<String>,
    ) -> Result<Self, BraidChessError> {
        let client = BraidClient::new().map_err(|e| BraidChessError::Http(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into(),
            game_id: game_id.into(),
            client,
        })
    }

    pub async fn subscribe_moves(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidChessError> {
        self.subscribe_inner(ChessResource::moves(&self.game_id))
            .await
    }

    pub async fn subscribe_engine(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidChessError> {
        self.subscribe_inner(ChessResource::engine(&self.game_id))
            .await
    }

    pub async fn subscribe_clock(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidChessError> {
        self.subscribe_inner(ChessResource::clock(&self.game_id))
            .await
    }

    pub async fn subscribe_chat(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidChessError> {
        self.subscribe_inner(ChessResource::chat(&self.game_id))
            .await
    }

    async fn subscribe_inner(
        &self,
        resource: ChessResource,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidChessError> {
        let url = resource.to_url(&self.base_url);
        debug!("[BRAID SUB] Subscribing to {}", url);

        let request = BraidRequest::new()
            .subscribe()
            .with_heartbeat(20)
            .with_retry(braid_http::client::RetryConfig::default().with_max_retries(5));

        let mut subscription: Subscription = self
            .client
            .subscribe(&url, request)
            .await
            .map_err(|e| BraidChessError::Http(e.to_string()))?;

        // Bridge: Subscription.next() → ChessMessage channel
        let (chess_tx, chess_rx) = async_channel::bounded::<ChessMessage>(256);

        let handle = tokio::spawn(async move {
            loop {
                let update = tokio::select! { _ = chess_tx.closed() => break, update = subscription.next() => update };
                match update {
                    Some(Ok(update)) => {
                        if let Some(msg) = decode_update(&update) {
                            if chess_tx.send(msg).await.is_err() {
                                debug!("[BRAID SUB] Receiver dropped – exiting bridge");
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        warn!("[BRAID SUB] Update error: {e}");
                        break;
                    }
                    None => {
                        // Routine long-poll closure is not a warning; the caller decides whether
                        // the disconnect was unexpected.
                        debug!("[BRAID SUB] Stream ended");
                        break;
                    }
                }
            }
        });

        Ok((chess_rx, handle))
    }
}

fn decode_update(update: &Update) -> Option<ChessMessage> {
    if let Some(body_str) = update.body_str() {
        return parse_chess_message(body_str);
    }
    if let Some(patches) = &update.patches {
        for patch in patches {
            if let Ok(content) = std::str::from_utf8(&patch.content) {
                if let Some(msg) = parse_chess_message(content) {
                    return Some(msg);
                }
            }
        }
    }
    None
}

fn parse_chess_message(s: &str) -> Option<ChessMessage> {
    match serde_json::from_str::<ChessMessage>(s) {
        Ok(msg) => Some(msg),
        Err(e) => {
            warn!("[BRAID SUB] JSON parse error: {e}");
            None
        }
    }
}
