use bevy::prelude::*;
use iroh::{EndpointId, SecretKey};
use iroh_gossip::api::Event as IrohEvent;
use std::path::PathBuf;
use std::time::Instant;
use futures_lite::StreamExt;
use braid_iroh::{BraidGameConfig, BraidIrohNode, DiscoveryConfig};
use braid_core::{Update, Version};

use crate::multiplayer::types::*;
use crate::multiplayer::traits::*;
use crate::multiplayer::network::protocol::NetworkMessage;

#[cfg(feature = "solana")]
use crate::game::events::{MoveMadeEvent, GameEndedEvent};
#[cfg(feature = "solana")]
use crate::game::resources::history::game_over::GameOverState;
#[cfg(feature = "solana")]
use crate::rendering::PieceType;

pub const GAME_TOPIC: &str = "/xfchess-game";

/// Initializes the Braid/Iroh networking layer in a background Tokio task.
pub fn initialize_braid_network(
    mut network_state: ResMut<BraidNetworkState>,
) {
    info!("Initializing Braid/Iroh networking layer");

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
    let (bootstrap_tx, mut bootstrap_rx) = tokio::sync::mpsc::unbounded_channel::<EndpointId>();

    network_state.event_receiver = Some(event_rx);
    network_state.message_sender = Some(msg_tx);
    network_state.bootstrap_sender = Some(bootstrap_tx);

    let event_tx_clone = event_tx.clone();

    bevy::tasks::IoTaskPool::get().spawn(async move {
        let (secret_key, raw_bytes) = load_or_generate_key();

        let config = BraidGameConfig {
            secret_key: Some(secret_key),
            discovery: DiscoveryConfig::Real,
            proxy_config: None,
            app_router: Default::default(),
            db: Default::default(),
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

        // Outgoing message loop
        bevy::tasks::IoTaskPool::get().spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let json = match serde_json::to_vec(&msg) {
                    Ok(b) => b,
                    Err(e) => {
                        error!("Failed to serialize NetworkMessage: {}", e);
                        continue;
                    }
                };
                let version = Version::new(uuid::Uuid::new_v4().to_string());
                let update = Update::snapshot(version, json.into());
                if let Err(e) = node_send.put(GAME_TOPIC, update).await {
                    error!("Failed to broadcast message: {}", e);
                }
            }
        });

        // Bootstrap loop
        bevy::tasks::IoTaskPool::get().spawn(async move {
            while let Some(peer_id) = bootstrap_rx.recv().await {
                if let Err(e) = node_bootstrap.join_peers(GAME_TOPIC, vec![peer_id]).await {
                    error!("Failed to join peer {}: {}", peer_id, e);
                }
            }
        });

        // Incoming gossip loop
        while let Some(result) = rx.next().await {
            match result {
                Ok(IrohEvent::NeighborUp(peer_id)) => {
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
                            role: NodeRole::Player,
                            connected_game: None,
                        }))
                        .ok();
                }
                Ok(IrohEvent::Received(msg)) => {
                    match serde_json::from_slice::<Update>(&msg.content) {
                        Ok(update) => {
                            if let Some(body) = update.body {
                                match serde_json::from_slice::<NetworkMessage>(&body) {
                                    Ok(net_msg) => {
                                        event_tx_clone.send(NetworkEvent::MessageReceived(net_msg)).ok();
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                        Err(_) => {
                            if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&msg.content) {
                                event_tx_clone.send(NetworkEvent::MessageReceived(net_msg)).ok();
                            }
                        }
                    }
                }
                Ok(IrohEvent::NeighborDown(peer_id)) => {
                    info!("GOSSIP NeighborDown: {}", peer_id);
                    let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                    event_tx_clone.send(NetworkEvent::PeerDisconnected(bs58_id)).ok();
                }
                _ => {}
            }
        }
    }).detach();
}

/// Polling system to read NetworkEvents from the background task and write them as Bevy events.
pub fn handle_network_events(
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
            NetworkEvent::GameInviteReceived(node_id, prefs) => {
                network_state.pending_invites.insert(node_id.clone(), prefs.clone());
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
                                wallet_address: "unknown".to_string(),
                                game_preferences: GamePreferences {
                                    stake_amount: 0.0,
                                    time_control: TimeControl {
                                        base_time_seconds: 600,
                                        increment_seconds: 0,
                                    },
                                    variant: ChessVariant::Standard,
                                },
                                last_seen: Instant::now(),
                                role: NodeRole::Player,
                                connected_game: Some(*game_id),
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
            NetworkEvent::GameEnded(_) => {
                network_state.active_session = None;
            }
            _ => {}
        }
        network_events.write(event);
    }
}

#[cfg(feature = "solana")]
pub fn feed_local_moves_to_rollup(
    mut move_events: MessageReader<MoveMadeEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    network_state: Res<BraidNetworkState>,
) {
    if network_state.active_session.is_none() {
        return;
    }

    for event in move_events.read() {
        if event.remote { continue; }

        let from_col = (b'a' + event.from.0) as char;
        let from_row = event.from.1 + 1;
        let to_col = (b'a' + event.to.0) as char;
        let to_row = event.to.1 + 1;

        let mut uci = format!("{}{}{}{}", from_col, from_row, to_col, to_row);

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

        rollup_manager.add_local_move(uci, event.next_fen.clone());
    }
}

#[cfg(feature = "solana")]
pub fn feed_remote_moves_to_rollup(
    mut remote_events: MessageReader<crate::game::events::RemoteMoveApplied>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    network_state: Res<BraidNetworkState>,
) {
    if network_state.active_session.is_none() { return; }
    if !rollup_manager.is_creator { return; }

    for event in remote_events.read() {
        rollup_manager.add_local_move(event.uci.clone(), event.next_fen.clone());
    }
}

#[cfg(feature = "solana")]
pub fn handle_session_info_from_network(
    mut network_events: MessageReader<NetworkEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut session_key_manager: ResMut<crate::multiplayer::rollup::session_keys::SessionKeyManager>,
    mut solana_state: Option<ResMut<crate::multiplayer::solana::integration::state::SolanaIntegrationState>>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
            game_id,
            player_pubkey,
            session_pubkey,
            ..
        }) = event
        {
            if game_id != &rollup_manager.game_id { continue; }
            if let Some(ref mut state) = solana_state {
                state.opponent_pubkey = Some(*player_pubkey);
            }
            session_key_manager.set_game_id(*game_id);

            if let Some(our_pubkey) = session_key_manager.get_session_pubkey() {
                use solana_sdk::pubkey::Pubkey;
                let peer_key: Pubkey = *session_pubkey;
                let our_key: Pubkey = our_pubkey;
                let (white_key, black_key) = if our_key.to_string() < peer_key.to_string() {
                    (our_key, peer_key)
                } else {
                    (peer_key, our_key)
                };
                rollup_manager.set_session_keys(white_key, black_key);
            }
        }
    }
}

#[cfg(feature = "solana")]
pub fn finalize_game_on_end(
    mut game_end_events: MessageReader<GameEndedEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut rollup_events: MessageWriter<crate::multiplayer::rollup::manager::RollupEvent>,
    network_state: Res<BraidNetworkState>,
) {
    for _event in game_end_events.read() {
        if network_state.active_session.is_none() { continue; }
        if let Some((moves, next_fens)) = rollup_manager.force_flush() {
            rollup_events.write(crate::multiplayer::rollup::manager::RollupEvent::GameEndBatch {
                game_id: rollup_manager.game_id,
                moves,
                next_fens,
            });
        }
    }
}

#[cfg(feature = "solana")]
pub fn emit_game_ended_event(
    game_over: Res<GameOverState>,
    mut game_ended_events: MessageWriter<GameEndedEvent>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut emitted: Local<bool>,
) {
    if !game_over.is_game_over() {
        *emitted = false;
        return;
    }
    if *emitted { return; }
    let game_id = match competitive.as_ref().and_then(|c| c.game_id) {
        Some(id) => id,
        None => return,
    };
    *emitted = true;

    let (winner, reason) = match *game_over {
        GameOverState::WhiteWon => (Some("white".to_string()), "checkmate"),
        GameOverState::WhiteWonByTime => (Some("white".to_string()), "timeout"),
        GameOverState::BlackWon => (Some("black".to_string()), "checkmate"),
        GameOverState::BlackWonByTime => (Some("black".to_string()), "timeout"),
        GameOverState::Stalemate => (None, "stalemate"),
        GameOverState::InsufficientMaterial => (None, "insufficient_material"),
        GameOverState::Playing => return,
    };

    game_ended_events.write(GameEndedEvent {
        game_id,
        winner,
        reason: reason.to_string(),
    });
}

pub fn load_or_generate_key() -> (SecretKey, [u8; 32]) {
    let key_file = if let Ok(env_path) = std::env::var("XFCHESS_IDENTITY") {
        PathBuf::from(env_path)
    } else {
        let sk = SecretKey::generate(&mut rand::rng());
        let bytes = sk.to_bytes();
        return (sk, bytes);
    };

    if let Ok(bytes) = std::fs::read(&key_file) {
        if bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let sk = SecretKey::from_bytes(&arr);
            return (sk, arr);
        }
    }

    let sk = SecretKey::generate(&mut rand::rng());
    let bytes = sk.to_bytes();
    let _ = std::fs::write(&key_file, bytes);
    (sk, bytes)
}
