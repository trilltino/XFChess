use bevy::prelude::*; // Events are in prelude
use futures_lite::StreamExt;
use iroh::{EndpointId, SecretKey};
use iroh_gossip::api::Event;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio;

use crate::engine::board_state::ChessEngine;
use crate::game::events::{GameEndedEvent, GameStartedEvent, MoveMadeEvent};
#[cfg(feature = "solana")]
use crate::game::resources::history::game_over::GameOverState;
use crate::rendering::pieces::PieceType;
use braid_core::{Patch, Update, Version};
use braid_iroh::{BraidGameConfig, BraidIrohNode, DiscoveryConfig};

pub mod network;
#[cfg(feature = "solana")]
pub mod solana;
#[cfg(feature = "solana")]
pub mod rollup;
pub mod ui;
#[cfg(feature = "solana")]
pub mod wager_state;

pub use network::*;
#[cfg(feature = "solana")]
pub use solana::*;
#[cfg(feature = "solana")]
pub use rollup::*;
pub use ui::*;
#[cfg(feature = "solana")]
pub use wager_state::*;

pub struct MultiplayerPlugin;

impl Plugin for MultiplayerPlugin {
    fn build(&self, app: &mut App) {
        // Register sub-plugins
        app.add_plugins(network::p2p::P2PConnectionPlugin);

        #[cfg(feature = "solana")]
        app.add_plugins((
            rollup::manager::EphemeralRollupPlugin,
            rollup::bridge::RollupNetworkBridgePlugin,
            solana::integration::SolanaIntegrationPlugin,
            solana::lobby::SolanaLobbyPlugin,
        ));

        // Initialize necessary resources
        app.init_resource::<BraidNetworkState>()
            .init_resource::<BraidGameSync>()
            .init_resource::<network::braid::BraidP2PConfig>()
            .add_message::<NetworkEvent>()
            .add_message::<GameStartedEvent>()
            .add_message::<GameEndedEvent>();

        #[cfg(feature = "solana")]
        app.init_resource::<rollup::session_keys::SessionKeyManager>();

        #[cfg(feature = "solana")]
        app.init_resource::<PendingFinalize>();

        #[cfg(feature = "solana")]
        app.add_systems(
            Update,
            (
                feed_local_moves_to_rollup,
                handle_session_info_from_network,
                finalize_game_on_end,
                emit_game_ended_event,
                // handle_pending_finalize removed — finalization now handled
                // by handle_game_end_undelegation in rollup_network_bridge.rs
                // via VPS session key (no wallet popup).
            ),
        );

        app.add_systems(Startup, initialize_braid_network)
            .add_systems(Update, handle_network_events);
    }
}

#[derive(Event, Message, Debug, Clone)]
pub enum NetworkEvent {
    NetworkInitialized {
        node_id: EndpointId,
        secret_key_bytes: [u8; 32],
    },
    PeerDiscovered(PeerInfo),
    GameInviteReceived(String, GamePreferences),
    GameInviteAccepted(String),
    WagerHandshake {
        node_id: String,
        game_id: u64,
    },
    MessageReceived(NetworkMessage),
    GameEnded(String),
    PeerConnected(String),
}

#[derive(Resource)]
pub struct BraidNetworkState {
    pub node_id: Option<EndpointId>,
    pub secret_key_bytes: Option<[u8; 32]>,
    pub connected: bool,
    pub discovered_peers: Vec<PeerInfo>,
    pub connected_peers: std::collections::HashSet<String>,
    pub active_session: Option<GameSession>,
    pub pending_invites: HashMap<String, GamePreferences>,
    pub event_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>>,
    pub message_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkMessage>>,
    pub bootstrap_sender: Option<tokio::sync::mpsc::UnboundedSender<EndpointId>>,
}

impl Default for BraidNetworkState {
    fn default() -> Self {
        Self {
            node_id: None,
            secret_key_bytes: None,
            connected: false,
            discovered_peers: Vec::new(),
            connected_peers: std::collections::HashSet::new(),
            active_session: None,
            pending_invites: HashMap::new(),
            event_receiver: None,
            message_sender: None,
            bootstrap_sender: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub wallet_address: String,
    pub game_preferences: GamePreferences,
    pub last_seen: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePreferences {
    pub stake_amount: f64,
    pub time_control: TimeControl,
    pub variant: ChessVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeControl {
    pub base_time_seconds: u32,
    pub increment_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChessVariant {
    Standard,
    Chess960,
    ThreeCheck,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerColor {
    White,
    Black,
}

#[derive(Debug, Clone)]
pub struct GameSession {
    pub session_id: String,
    pub opponent: PeerInfo,
    pub stake_amount: f64,
    pub confirmed: bool,
    pub started: bool,
    pub game_state: Option<MultiplayerGameState>,
}

#[derive(Debug, Clone)]
pub struct MultiplayerGameState {
    pub game_id: u64,
    pub my_color: PlayerColor,
    pub initial_fen: String,
    pub last_active: Instant,
}

#[derive(Resource)]
pub struct BraidGameSync {
    pub pending_patches: Vec<Patch>,
}

#[derive(Resource, Clone)]
pub struct TokioRuntime(pub tokio::runtime::Handle);

impl Default for BraidGameSync {
    fn default() -> Self {
        Self {
            pending_patches: Vec::new(),
        }
    }
}

const GAME_TOPIC: &str = "/xfchess-game";

fn initialize_braid_network(
    mut network_state: ResMut<BraidNetworkState>,
    tokio_runtime: Res<TokioRuntime>,
) {
    info!("Initializing Braid/Iroh networking layer");

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
    let (bootstrap_tx, mut bootstrap_rx) = tokio::sync::mpsc::unbounded_channel::<EndpointId>();

    network_state.event_receiver = Some(event_rx);
    network_state.message_sender = Some(msg_tx);
    network_state.bootstrap_sender = Some(bootstrap_tx);

    let event_tx_clone = event_tx.clone();

    let tokio_runtime_inner = tokio_runtime.clone();
    tokio_runtime.0.spawn(async move {
        let (secret_key, raw_bytes) = load_or_generate_key();

        let config = BraidGameConfig {
            secret_key: Some(secret_key),
            discovery: DiscoveryConfig::Real,
            proxy_config: None,
        };

        let node = match BraidIrohNode::spawn(config).await {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to spawn BraidIrohNode: {}", e);
                return;
            }
        };

        let node_id = node.node_id();
        event_tx_clone
            .send(NetworkEvent::NetworkInitialized {
                node_id,
                secret_key_bytes: raw_bytes,
            })
            .ok();

        let mut rx = match node.subscribe(GAME_TOPIC, vec![]).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to subscribe to gossip topic: {}", e);
                return;
            }
        };

        let node_arc = std::sync::Arc::new(node);
        let node_send = node_arc.clone();
        let node_bootstrap = node_arc.clone();

        tokio_runtime_inner.0.spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let json = match serde_json::to_vec(&msg) {
                    Ok(b) => b,
                    Err(e) => {
                        error!("Failed to serialize NetworkMessage: {}", e);
                        continue;
                    }
                };
                let version = Version::new(uuid::Uuid::new_v4().to_string());
                let update = Update::snapshot(version, json);
                if let Err(e) = node_send.put(GAME_TOPIC, update).await {
                    error!("Failed to broadcast message: {}", e);
                }
            }
        });

        tokio_runtime_inner.0.spawn(async move {
            while let Some(peer_id) = bootstrap_rx.recv().await {
                if let Err(e) = node_bootstrap.join_peers(GAME_TOPIC, vec![peer_id]).await {
                    error!("Failed to join peer {}: {}", peer_id, e);
                }
            }
        });

        while let Some(result) = rx.next().await {
            match result {
                Ok(Event::NeighborUp(peer_id)) => {
                    info!("GOSSIP NeighborUp: {}", peer_id);
                    let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                    event_tx_clone
                        .send(NetworkEvent::PeerConnected(bs58_id.clone()))
                        .ok();

                    event_tx_clone
                        .send(NetworkEvent::PeerDiscovered(PeerInfo {
                            node_id: bs58_id.clone(),
                            wallet_address: format!("sol:{}...", &bs58_id[..8]),
                            game_preferences: GamePreferences {
                                stake_amount: 0.5,
                                time_control: TimeControl {
                                    base_time_seconds: 600,
                                    increment_seconds: 2,
                                },
                                variant: ChessVariant::Standard,
                            },
                            last_seen: Instant::now(),
                        }))
                        .ok();
                }
                Ok(Event::Received(msg)) => {
                    trace!(
                        "GOSSIP RECEIVED RAW BYTES: {} bytes from {}",
                        msg.content.len(),
                        msg.delivered_from
                    );
                    // msg.content contains a serialized braid_core::Update
                    match serde_json::from_slice::<Update>(&msg.content) {
                        Ok(update) => {
                            if let Some(body) = update.body {
                                match serde_json::from_slice::<NetworkMessage>(&body) {
                                    Ok(net_msg) => {
                                        trace!(
                                            "GOSSIP Parsed NetworkMessage from Update body: {:?}",
                                            net_msg.game_id()
                                        );
                                        event_tx_clone
                                            .send(NetworkEvent::MessageReceived(net_msg))
                                            .ok();
                                    }
                                    Err(e) => {
                                        trace!(
                                            "Failed to parse NetworkMessage from Update body: {}",
                                            e
                                        );
                                    }
                                }
                            } else {
                                trace!("Received Update had no body");
                            }
                        }
                        Err(e) => {
                            // Fallback just in case it was sent directly as a NetworkMessage
                            match serde_json::from_slice::<NetworkMessage>(&msg.content) {
                                Ok(net_msg) => {
                                    trace!(
                                        "GOSSIP Parsed NetworkMessage from fallback: {:?}",
                                        net_msg.game_id()
                                    );
                                    event_tx_clone
                                        .send(NetworkEvent::MessageReceived(net_msg))
                                        .ok();
                                }
                                Err(_) => {
                                    trace!("Failed to parse message from gossip: {}", e);
                                    if let Ok(text) = String::from_utf8(msg.content.to_vec()) {
                                        trace!("Raw gossip content: {}", text);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::NeighborDown(peer_id)) => {
                    info!("GOSSIP NeighborDown: {}", peer_id);
                }
                Ok(Event::Lagged) => {
                    warn!("GOSSIP Lagged!");
                }
                Err(e) => {
                    error!("Gossip stream error: {}", e);
                    break;
                }
            }
        }
    });
}

fn handle_network_events(
    mut network_state: ResMut<BraidNetworkState>,
    mut network_events: MessageWriter<NetworkEvent>,
) {
    let events: Vec<NetworkEvent> = {
        if let Some(ref mut receiver) = network_state.event_receiver {
            let mut buf = Vec::new();
            while let Ok(ev) = receiver.try_recv() {
                buf.push(ev);
            }
            buf
        } else {
            Vec::new()
        }
    };

    for event in events {
        match &event {
            NetworkEvent::NetworkInitialized {
                node_id,
                secret_key_bytes,
            } => {
                network_state.node_id = Some(*node_id);
                network_state.secret_key_bytes = Some(*secret_key_bytes);
                network_state.connected = true;
                info!("Braid network initialized with node ID: {}", node_id);
            }
            NetworkEvent::PeerDiscovered(peer_info) => {
                if !network_state
                    .discovered_peers
                    .iter()
                    .any(|p| p.node_id == peer_info.node_id)
                {
                    info!("New peer discovered: {}", peer_info.node_id);
                    network_state.discovered_peers.push(peer_info.clone());
                }
            }
            NetworkEvent::WagerHandshake {
                node_id: _,
                game_id,
            } => {
                info!("Wager handshake received for game {}", game_id);
            }
            NetworkEvent::GameInviteReceived(node_id, prefs) => {
                network_state
                    .pending_invites
                    .insert(node_id.clone(), prefs.clone());
            }
            NetworkEvent::GameInviteAccepted(_) => {
                if let Some(session) = &mut network_state.active_session {
                    session.confirmed = true;
                    session.started = true;
                }
            }
            NetworkEvent::MessageReceived(msg) => {
                match msg {
                    NetworkMessage::GameInvite {
                        game_id: _,
                        from_node,
                        from_wallet,
                    } => {
                        info!(
                            "Received GameInvite from {} (wallet: {})",
                            from_node, from_wallet
                        );
                        network_state.pending_invites.insert(
                            from_node.clone(),
                            GamePreferences {
                                stake_amount: 0.0,
                                time_control: TimeControl {
                                    base_time_seconds: 600,
                                    increment_seconds: 0,
                                },
                                variant: ChessVariant::Standard,
                            },
                        );
                    }
                    NetworkMessage::InviteResponse { game_id, accepted } => {
                        info!(
                            "Received InviteResponse for game {}: accepted={}",
                            game_id, accepted
                        );
                        if *accepted {
                            if let Some(session) = &mut network_state.active_session {
                                session.confirmed = true;
                                session.started = true;
                            }
                        }
                    }
                    NetworkMessage::GameStart {
                        game_id,
                        white_player,
                        black_player,
                        initial_fen,
                    } => {
                        info!(
                            "Received GameStart for game {}: {} vs {}",
                            game_id, white_player, black_player
                        );
                        let my_node_id = network_state
                            .node_id
                            .as_ref()
                            .map(|id| bs58::encode(id.as_bytes()).into_string())
                            .unwrap_or_default();
                        let my_color = if white_player == &my_node_id {
                            PlayerColor::White
                        } else {
                            PlayerColor::Black
                        };

                        network_state.active_session = Some(GameSession {
                            session_id: game_id.to_string(),
                            opponent: PeerInfo {
                                node_id: if my_color == PlayerColor::White {
                                    black_player.clone()
                                } else {
                                    white_player.clone()
                                },
                                wallet_address: "unknown".to_string(), // Could be improved
                                game_preferences: GamePreferences {
                                    stake_amount: 0.0,
                                    time_control: TimeControl {
                                        base_time_seconds: 600,
                                        increment_seconds: 0,
                                    },
                                    variant: ChessVariant::Standard,
                                },
                                last_seen: Instant::now(),
                            },
                            stake_amount: 0.0,
                            confirmed: true,
                            started: true,
                            game_state: Some(MultiplayerGameState {
                                game_id: *game_id,
                                my_color,
                                initial_fen: initial_fen.clone(),
                                last_active: Instant::now(),
                            }),
                        });
                    }
                    _ => {}
                }
            }
            NetworkEvent::GameEnded(result) => {
                info!("Game ended: {}", result);
                network_state.active_session = None;
            }
            NetworkEvent::PeerConnected(_) => {}
        }
        network_events.write(event);
    }
}

/// Converts each `MoveMadeEvent` to a UCI string and feeds it into the
/// `EphemeralRollupManager` as a local move, pulling the new FEN from `ChessEngine`.
#[cfg(feature = "solana")]
fn feed_local_moves_to_rollup(
    mut move_events: MessageReader<MoveMadeEvent>,
    mut rollup_manager: ResMut<rollup::manager::EphemeralRollupManager>,
    engine: Res<ChessEngine>,
    network_state: Res<BraidNetworkState>,
) {
    // Only active when we are in a networked session
    if network_state.active_session.is_none() {
        return;
    }

    for event in move_events.read() {
        // Build UCI notation: e.g. "e2e4" from (col, row) coords
        let from_col = (b'a' + event.from.1) as char;
        let from_row = event.from.0 + 1;
        let to_col = (b'a' + event.to.1) as char;
        let to_row = event.to.0 + 1;

        let mut uci = format!("{}{}{}{}", from_col, from_row, to_col, to_row);

        // Append promotion piece character when applicable
        if let Some(promo) = event.promotion {
            let promo_char = match promo {
                PieceType::Queen => 'q',
                PieceType::Rook => 'r',
                PieceType::Bishop => 'b',
                PieceType::Knight => 'n',
                _ => 'q',
            };
            uci.push(promo_char);
        }

        let fen = engine.current_fen().to_string();
        info!("[ROLLUP] Local move {} → rollup (fen: {})", uci, fen);
        rollup_manager.add_local_move(uci, fen);
    }
}

/// Receives `NetworkMessage::SessionInfo` from peers and records their session pubkey
/// in the `EphemeralRollupManager` session_keys slot.
#[cfg(feature = "solana")]
fn handle_session_info_from_network(
    mut network_events: MessageReader<NetworkEvent>,
    mut rollup_manager: ResMut<rollup::manager::EphemeralRollupManager>,
    mut session_key_manager: ResMut<rollup::session_keys::SessionKeyManager>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
            game_id,
            player_pubkey,
            session_pubkey,
            expires_at: _,
        }) = event
        {
            if *game_id != rollup_manager.game_id {
                continue;
            }
            info!(
                "[SESSION] Received SessionInfo for game {} — peer session key: {}",
                game_id, session_pubkey
            );
            // Update session_key_manager game id so it manages the correct slot
            session_key_manager.set_game_id(*game_id);

            // Store both keys (our session_pubkey is set from solana_integration init).
            // We use the peer's session_pubkey as the second key; our own is already set.
            if let Some(our_pubkey) = session_key_manager.get_session_pubkey() {
                // Determine white/black from player_pubkey ordering (lexicographic)
                use solana_sdk::pubkey::Pubkey;
                let peer_key: Pubkey = *session_pubkey;
                let our_key: Pubkey = our_pubkey;
                let (white_key, black_key) = if our_key.to_string() < peer_key.to_string() {
                    (our_key, peer_key)
                } else {
                    (peer_key, our_key)
                };
                rollup_manager.set_session_keys(white_key, black_key);
                info!(
                    "[SESSION] Rollup session keys set — white: {}, black: {}",
                    white_key, black_key
                );
            }
        }
    }
}

/// On `GameEndedEvent`, force-flush any pending rollup batch so all moves are
/// committed on-chain before `finalize_game_ix` runs.
#[cfg(feature = "solana")]
fn finalize_game_on_end(
    mut game_end_events: MessageReader<GameEndedEvent>,
    mut rollup_manager: ResMut<rollup::manager::EphemeralRollupManager>,
    mut rollup_events: MessageWriter<rollup::manager::RollupEvent>,
    network_state: Res<BraidNetworkState>,
) {
    for event in game_end_events.read() {
        if network_state.active_session.is_none() {
            continue;
        }

        info!(
            "[ROLLUP] Game ended (winner: {:?}) — forcing final batch flush",
            event.winner
        );

        if let Some((moves, next_fens)) = rollup_manager.force_flush() {
            rollup_events.write(rollup::manager::RollupEvent::BatchReady {
                game_id: rollup_manager.game_id,
                moves,
                next_fens,
            });
        }
    }
}

/// Tracks a finalization that must be submitted after undelegation completes.
#[cfg(feature = "solana")]
#[derive(Resource, Default)]
pub struct PendingFinalize {
    pub game_id: Option<u64>,
    /// 0 = white wins, 1 = black wins, 2 = draw
    pub result_code: u8,
    pub ready: bool,
    pub submitted: bool,
}

/// Watch `GameOverState` and emit `GameEndedEvent` once per game over.
///
/// Uses a `Local<bool>` guard so it fires exactly once per state transition.
#[cfg(feature = "solana")]
fn emit_game_ended_event(
    game_over: Res<GameOverState>,
    mut game_ended_events: MessageWriter<GameEndedEvent>,
    mut pending: ResMut<PendingFinalize>,
    competitive: Option<Res<solana::addon::CompetitiveMatchState>>,
    mut emitted: Local<bool>,
) {
    if !game_over.is_game_over() {
        *emitted = false;
        return;
    }
    if *emitted {
        return;
    }
    let game_id = match competitive.as_ref().and_then(|c| c.game_id) {
        Some(id) => id,
        None => return,
    };
    *emitted = true;

    let (winner, reason, result_code) = match *game_over {
        GameOverState::WhiteWon => (Some("white".to_string()), "checkmate", 0u8),
        GameOverState::WhiteWonByTime => (Some("white".to_string()), "timeout", 0u8),
        GameOverState::BlackWon => (Some("black".to_string()), "checkmate", 1u8),
        GameOverState::BlackWonByTime => (Some("black".to_string()), "timeout", 1u8),
        GameOverState::Stalemate => (None, "stalemate", 2u8),
        GameOverState::InsufficientMaterial => (None, "insufficient_material", 2u8),
        GameOverState::Playing => return,
    };

    info!(
        "[FINALIZE] Game {} over — result_code={} winner={:?} reason={}",
        game_id, result_code, winner, reason
    );

    pending.game_id = Some(game_id);
    pending.result_code = result_code;
    pending.ready = false;
    pending.submitted = false;

    game_ended_events.write(GameEndedEvent {
        game_id,
        winner,
        reason: reason.to_string(),
    });
}

/// Submit `finalize_game` on-chain once undelegation has completed.
///
/// The system polls `MagicBlockResolver::is_delegated()` — once the game
/// is no longer delegated the final game state has been committed back to
/// Solana devnet and it is safe to call `finalize_game`.
#[cfg(feature = "solana")]
fn handle_pending_finalize(
    mut pending: ResMut<PendingFinalize>,
    magicblock_resolver: Option<Res<rollup::magicblock::MagicBlockResolver>>,
    solana_state: Option<Res<solana::integration::state::SolanaIntegrationState>>,
    network_state: Res<BraidNetworkState>,
) {
    if pending.submitted {
        return;
    }
    let game_id = match pending.game_id {
        Some(id) => id,
        None => return,
    };
    if network_state.active_session.is_none() {
        return;
    }

    let magicblock_resolver = match magicblock_resolver {
        Some(r) => r,
        None => return,
    };

    // Wait until the game is no longer delegated (ER committed state back).
    if magicblock_resolver.is_delegated() {
        if !pending.ready {
            pending.ready = true;
            info!("[FINALIZE] Undelegation pending — waiting before finalize for game {}", game_id);
        }
        return;
    }

    let solana = match solana_state {
        Some(s) => s,
        None => return,
    };
    let (wallet_pubkey, program_id) = match solana.wallet_pubkey {
        Some(pk) => (pk, solana.program_id),
        None => {
            warn!("[FINALIZE] Wallet not connected — cannot finalize game {}", game_id);
            return;
        }
    };

    let result_code = pending.result_code;
    pending.submitted = true;

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use solana_client::rpc_client::RpcClient;
            use crate::multiplayer::solana::integration::DEVNET_RPC_URL;
            use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
            use crate::solana::instructions::finalize_game_ix;

            let rpc = RpcClient::new(DEVNET_RPC_URL.to_string());
            let game_pda = solana_sdk::pubkey::Pubkey::find_program_address(
                &[crate::solana::instructions::GAME_SEED, &game_id.to_le_bytes()],
                &program_id,
            )
            .0;

            // Read on-chain game account to extract white + black pubkeys.
            let data = match rpc.get_account_data(&game_pda) {
                Ok(d) => d,
                Err(e) => {
                    error!("[FINALIZE] Failed to fetch game account for {}: {}", game_id, e);
                    return;
                }
            };
            // Layout: disc(8) + game_id(8) + white(32) + black(32) + ...
            if data.len() < 8 + 8 + 32 + 32 {
                error!("[FINALIZE] Game account data too short");
                return;
            }
            let white_pubkey = solana_sdk::pubkey::Pubkey::from(<[u8; 32]>::try_from(&data[16..48]).unwrap());
            let black_pubkey = solana_sdk::pubkey::Pubkey::from(<[u8; 32]>::try_from(&data[48..80]).unwrap());

            let ix = match finalize_game_ix(program_id, wallet_pubkey, game_id, result_code, white_pubkey, black_pubkey) {
                Ok(ix) => ix,
                Err(e) => {
                    error!("[FINALIZE] Failed to build finalize_game_ix: {}", e);
                    return;
                }
            };

            match sign_and_send_via_tauri(DEVNET_RPC_URL, wallet_pubkey, &[ix], &[]) {
                Ok(sig) => info!("[FINALIZE] Game {} finalized on-chain: {}", game_id, sig),
                Err(e) => error!("[FINALIZE] finalize_game failed for {}: {}", game_id, e),
            }
        })
        .detach();
}

fn load_or_generate_key() -> (SecretKey, [u8; 32]) {
    // Allow overriding identity file via environment variable for multi-instance testing
    // Example: XFCHESS_IDENTITY=keys/player1.key cargo run
    let key_file = if let Ok(env_path) = std::env::var("XFCHESS_IDENTITY") {
        PathBuf::from(env_path)
    } else {
        // For production, always generate random key - don't save to file
        // This ensures each run gets a fresh random node ID
        let sk = SecretKey::generate(&mut rand::rng());
        let bytes = sk.to_bytes();
        info!("Generated new random identity (not saved to file)");
        return (sk, bytes);
    };

    // Ensure parent directory exists (e.g., for keys/ folder)
    if let Some(parent) = key_file.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    if let Ok(bytes) = std::fs::read(&key_file) {
        if bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let sk = SecretKey::from_bytes(&arr);
            info!("Loaded identity from {:?}", key_file);
            return (sk, arr);
        }
    }

    let sk = SecretKey::generate(&mut rand::rng());
    let bytes = sk.to_bytes();
    if let Err(e) = std::fs::write(&key_file, bytes) {
        warn!("Failed to write identity to {:?}: {}", key_file, e);
    } else {
        info!("Generated new identity at {:?}", key_file);
    }
    (sk, bytes)
}
