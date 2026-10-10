use crate::error::BraidChessError;
use crate::message::ChessMessage;
use crate::resource::ChessResource;
use async_channel::Receiver;
use braid_http::client::Subscription;
use braid_http::types::{BraidRequest, Update};
use braid_http::BraidClient;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

pub struct ChessSubscriber {
    base_url: String,
    game_id: String,
    client: BraidClient,
}

impl ChessSubscriber {
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

        let request = BraidRequest::new().subscribe();

        let mut subscription: Subscription = self
            .client
            .subscribe(&url, request)
            .await
            .map_err(|e| BraidChessError::Http(e.to_string()))?;

        // Bridge: Subscription.next() → ChessMessage channel
        let (chess_tx, chess_rx) = async_channel::unbounded::<ChessMessage>();

        let handle = tokio::spawn(async move {
            loop {
                match subscription.next().await {
                    Some(Ok(update)) => {
                        if let Some(msg) = decode_update(&update) {
                            if chess_tx.send(msg).await.is_err() {
                                debug!("[BRAID SUB] Receiver dropped – exiting bridge");
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // Warn at most once per 30 seconds for routine heartbeat timeouts.
                        static LAST_WARN_NS: AtomicU64 = AtomicU64::new(0);
                        const WARN_COOLDOWN_SECS: u64 = 30;
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let last = LAST_WARN_NS.load(Ordering::Relaxed);
                        if now.saturating_sub(last) >= WARN_COOLDOWN_SECS {
                            LAST_WARN_NS.store(now, Ordering::Relaxed);
                            warn!("[BRAID SUB] Update error: {}", e);
                        } else {
                            debug!("[BRAID SUB] Update error (throttled): {}", e);
                        }
                        if matches!(e, braid_http::BraidError::SubscriptionClosed) {
                            error!("[BRAID SUB] Server closed the connection");
                            break;
                        }
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
            warn!("[BRAID SUB] JSON parse error: {} | body: {:?}", e, s);
            None
        }
    }
}
