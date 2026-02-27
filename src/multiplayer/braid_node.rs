use bevy::prelude::*;
use braid_uri::{ChessMessage, ChessPublisher, ChessSubscriber};
use crossbeam_channel::{bounded, Receiver};
use serde::{Deserialize, Serialize};

/// Represents the synchronized state of a chess game over Braid.
/// This matches the legacy structure but we now prefer ChessMessage.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Reflect)]
#[reflect(Default)]
pub struct BraidGameState {
    pub fen: String,
    pub last_move: Option<String>,
    pub is_white_turn: bool,
    pub status: GameStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Reflect)]
pub enum GameStatus {
    #[default]
    Playing,
    Checkmate,
    Stalemate,
    Resigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum BraidConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Subscribed,
    Error,
}

#[derive(Resource, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct BraidP2PConfig {
    pub base_url: String,
    pub game_id: String,
    pub active: bool,
}

/// Message for notifying Bevy about network incoming states
#[derive(Message, Debug)]
pub struct NetworkGameStateUpdated(pub BraidGameState);

/// A Bevy Resource to hold the local Braid node configuration/handles.
#[derive(Resource)]
pub struct BraidNodeManager {
    pub connection_status: BraidConnectionStatus,
    /// Crossbeam channel for Bevy to poll incoming ChessMessages
    pub incoming_chess_rx: Option<Receiver<ChessMessage>>,
    /// Sender to push local moves/events to the background publisher task
    pub outgoing_tx: Option<tokio::sync::mpsc::Sender<ChessMessage>>,
    /// Stockfish AI move receiver (UCI strings)
    pub incoming_moves_rx: Option<Receiver<String>>,
    /// FEN sender for Stockfish sidecar
    pub sidecar_fen_tx: Option<std::sync::mpsc::Sender<String>>,
}

impl BraidNodeManager {
    pub fn initialize_braid_session(
        &mut self,
        braid_url: &str,
        tokio_runtime: &crate::multiplayer::TokioRuntime,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Extract game ID from URL
        let parts: Vec<&str> = braid_url.split('/').collect();
        let game_id = parts.last().unwrap_or(&"default").to_string();
        let base_url = braid_url
            .trim_end_matches(&format!("/{}", game_id))
            .to_string();

        // Set up channels
        let (chess_tx, chess_rx) = bounded(100);
        self.incoming_chess_rx = Some(chess_rx);

        let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<ChessMessage>(100);
        self.outgoing_tx = Some(out_tx);

        // Update connection status
        self.connection_status = BraidConnectionStatus::Connecting;

        // Spawn background task for the Braid session
        let url = base_url.clone();
        let id = game_id.clone();
        tokio_runtime.0.spawn(async move {
            info!("[Braid Session] Starting Braid session for game {}...", id);

            // Setup Subscriber
            let subscriber = match ChessSubscriber::new(&url, &id) {
                Ok(s) => s,
                Err(e) => {
                    error!("[Braid Session] Failed to create subscriber: {}", e);
                    return;
                }
            };

            let (sub_rx, _sub_handle) = match subscriber.subscribe_moves().await {
                Ok(res) => res,
                Err(e) => {
                    error!("[Braid Session] Failed to subscribe: {}", e);
                    return;
                }
            };

            // Setup Publisher
            let mut publisher = match ChessPublisher::new(&url, &id) {
                Ok(p) => p,
                Err(e) => {
                    error!("[Braid Session] Failed to create publisher: {}", e);
                    return;
                }
            };

            // Bridge loops for receiving and sending messages
            let sub_bridge = async {
                while let Ok(msg) = sub_rx.recv().await {
                    debug!("[Braid Session] Incoming msg: {:?}", msg);
                    let _ = chess_tx.send(msg);
                }
            };

            let pub_bridge = async {
                while let Some(msg) = out_rx.recv().await {
                    debug!("[Braid Session] Publishing msg: {:?}", msg);
                    let res = match msg {
                        ChessMessage::Move(payload) => publisher.publish_move(&payload).await,
                        ChessMessage::Resign { player } => publisher.publish_resign(&player).await,
                        ChessMessage::OfferDraw { player } => {
                            publisher.publish_offer_draw(&player).await
                        }
                        _ => Ok(()), // Ignore others for now
                    };
                    if let Err(e) = res {
                        warn!("[Braid Session] Publish failed: {}", e);
                    }
                }
            };

            tokio::join!(sub_bridge, pub_bridge);
            info!("[Braid Session] Session task exited");
        });

        Ok(())
    }
}

impl Default for BraidNodeManager {
    fn default() -> Self {
        Self {
            connection_status: BraidConnectionStatus::Disconnected,
            incoming_chess_rx: None,
            outgoing_tx: None,
            incoming_moves_rx: None,
            sidecar_fen_tx: None,
        }
    }
}

// ... rest of the file stays the same ...
