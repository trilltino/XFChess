//! Full Braid-HTTP subscriber for chess game move streams.
//!
//! [`ChessSubscriber`] opens a long-lived `GET` with `Subscribe: keep-alive`
//! and yields each [`ChessMessage`] as it arrives via server-sent Braid updates.
//!
//! Uses `braid-http`'s [`BraidClient`] which handles multipart Braid framing,
//! reconnection, and heartbeat detection.
//!
//! # Example
//!
//! ```rust,no_run
//! use braid_uri::{ChessSubscriber, ChessMessage};
//!
//! #[tokio::main]
//! async fn main() {
//!     let sub = ChessSubscriber::new("http://localhost:3000", "ABCD42").unwrap();
//!     let (rx, _handle) = sub.subscribe_moves().await.unwrap();
//!
//!     while let Ok(msg) = rx.recv().await {
//!         match msg {
//!             ChessMessage::Move(mv) => println!("Opponent played: {}", mv.uci),
//!             ChessMessage::Resign { player } => println!("{} resigned", player),
//!             _ => {}
//!         }
//!     }
//! }
//! ```

use crate::error::BraidUriError;
use crate::message::ChessMessage;
use crate::uri::ChessUri;
use async_channel::Receiver;
use braid_http::client::Subscription;
use braid_http::types::{BraidRequest, Update};
use braid_http::BraidClient;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Subscribes to Braid resource streams and decodes them as [`ChessMessage`]s.
///
/// One [`ChessSubscriber`] per game.  Call [`subscribe_moves`], [`subscribe_engine`],
/// or [`subscribe_clock`] to open streams for each sub-resource.
pub struct ChessSubscriber {
    base_url: String,
    game_id: String,
    client: BraidClient,
}

impl ChessSubscriber {
    /// Create a subscriber for the given game.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be initialised
    /// (e.g. TLS configuration issue).
    pub fn new(
        base_url: impl Into<String>,
        game_id: impl Into<String>,
    ) -> Result<Self, BraidUriError> {
        let client = BraidClient::new().map_err(|e| BraidUriError::Http(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into(),
            game_id: game_id.into(),
            client,
        })
    }

    // ─── Subscription methods ────────────────────────────────────────────────

    /// Subscribe to the move stream (`/game/{id}/moves`).
    ///
    /// Returns `(receiver, task_handle)`.  Receive [`ChessMessage`]s from the
    /// `receiver`; drop it to stop the subscription background task.
    pub async fn subscribe_moves(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidUriError> {
        self.subscribe_inner(ChessUri::moves(&self.game_id)).await
    }

    /// Subscribe to engine hint stream (`/game/{id}/engine`).
    pub async fn subscribe_engine(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidUriError> {
        self.subscribe_inner(ChessUri::engine(&self.game_id)).await
    }

    /// Subscribe to clock state stream (`/game/{id}/clock`).
    pub async fn subscribe_clock(
        &self,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidUriError> {
        self.subscribe_inner(ChessUri::clock(&self.game_id)).await
    }

    // ─── Internal ────────────────────────────────────────────────────────────

    async fn subscribe_inner(
        &self,
        uri: ChessUri,
    ) -> Result<(Receiver<ChessMessage>, JoinHandle<()>), BraidUriError> {
        let url = format!("{}{}", self.base_url, uri.to_http_path());
        info!("[BRAID SUB] Subscribing to {}", url);

        // Build a subscribe request (BraidClient handles the Subscribe: keep-alive header)
        let request = BraidRequest::new().subscribe();

        let mut subscription: Subscription = self
            .client
            .subscribe(&url, request)
            .await
            .map_err(|e| BraidUriError::Http(e.to_string()))?;

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
                        warn!("[BRAID SUB] Update error: {}", e);
                        // braid-http will return Timeout on heartbeat miss; other errors
                        // are usually transient unless the connection fully died.
                        if matches!(e, braid_http::BraidError::SubscriptionClosed) {
                            error!("[BRAID SUB] Server closed the connection");
                            break;
                        }
                    }
                    None => {
                        info!("[BRAID SUB] Stream ended");
                        break;
                    }
                }
            }
        });

        Ok((chess_rx, handle))
    }
}

// ─── Decode helpers ──────────────────────────────────────────────────────────

/// Decode a Braid [`Update`] into a [`ChessMessage`].
///
/// Tries the snapshot body first, then the first patch's value body.
fn decode_update(update: &Update) -> Option<ChessMessage> {
    if let Some(body_str) = update.body_str() {
        return parse_chess_message(body_str);
    }
    if let Some(patches) = &update.patches {
        for patch in patches {
            if let Some(content) = patch.content_str() {
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
