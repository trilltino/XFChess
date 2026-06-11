use bevy::prelude::*;
use iroh::{EndpointId, SecretKey};
use iroh_gossip::api::Event as IrohEvent;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Instant;
use futures_lite::StreamExt;
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig};
use braid_core::{Update, Version};
use braid_uri;

use crate::multiplayer::types::*;
use crate::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};
use crate::game::events::ResignEvent;
use crate::multiplayer::TokioRuntime;

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
    tokio_runtime: Res<TokioRuntime>,
) {
    if network_state.connected {
        return;
    }

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
    let (bootstrap_tx, mut bootstrap_rx) = tokio::sync::mpsc::unbounded_channel::<EndpointId>();
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    network_state.event_receiver = Some(event_rx);
    network_state.message_sender = Some(msg_tx);
    network_state.bootstrap_sender = Some(bootstrap_tx);
    network_state.subscription_sender = Some(sub_tx);

    let session_signing_key = network_state.session_signing_key;
    let event_tx_clone = event_tx.clone();

    tokio_runtime.0.spawn(async move {
        info!("[NET] Starting Iroh node task...");
        let (secret_key, raw_bytes) = load_or_generate_key();
        // Derive node ID before consuming the key so we can set it as the
        // proxy's default_peer (browser spectators → local iroh node).
        let derived_node_id: EndpointId = secret_key.public();

        let braid_data_dir = dirs::data_local_dir()
            .map(|d| d.join("xfchess").join("braid"))
            .or_else(|| Some(std::path::PathBuf::from("braid-data")));

        let config = BraidIrohConfig {
            secret_key: Some(secret_key),
            discovery: DiscoveryConfig::Real,
            proxy_config: Some(braid_iroh::ProxyConfig {
                listen_addr: "127.0.0.1:8181".parse().expect("static addr"),
                default_peer: derived_node_id,
            }),
            data_dir: braid_data_dir,
        };

        let node = match BraidIrohNode::spawn(config).await {
            Ok(n) => n,
            Err(e) => {
                error!("❌ Failed to spawn BraidIrohNode: {}", e);
                event_tx_clone.send(NetworkEvent::PeerDisconnected(format!("Spawn failed: {}", e))).ok();
                return;
            }
        };

        let node_id = node.node_id();
        info!("[NET] BraidIrohNode spawned successfully (ID: {})", node_id);
        
        event_tx_clone
            .send(NetworkEvent::NetworkInitialized {
                node_id,
                secret_key_bytes: raw_bytes,
            })
            .ok();

        let rx = match node.subscribe(GAME_TOPIC, vec![]).await {
            Ok(r) => r,
            Err(e) => {
                error!("❌ Failed to subscribe to gossip topic: {}", e);
                return;
            }
        };

        let node_arc = std::sync::Arc::new(node);
        let node_send = node_arc.clone();
        let node_bootstrap = node_arc.clone();
        let node_sub = node_arc.clone();

        // 1. Outgoing message loop
        let event_tx_error = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                // Determine topic before msg is potentially consumed by signing.
                let topic = match &msg {
                    NetworkMessage::GameInvite { .. } |
                    NetworkMessage::InviteResponse { .. } |
                    NetworkMessage::GameStart { .. } => GAME_TOPIC.to_string(),
                    _ => format!("{}/{}", GAME_TOPIC, msg.game_id()),
                };

                // Serialize with a 1-byte version prefix:
                // 0x02 = bincode-encoded SignedNetworkMessage (secure path)
                // 0x01 = JSON-encoded NetworkMessage (legacy plain path)
                let payload_bytes: Vec<u8> = if let Some(ref sk) = session_signing_key {
                    let signed = SignedNetworkMessage::sign(msg, sk);
                    match bincode::serialize(&signed) {
                        Ok(mut b) => {
                            let mut out = vec![0x02];
                            out.append(&mut b);
                            out
                        }
                        Err(e) => {
                            error!("Failed to bincode SignedNetworkMessage: {}", e);
                            continue;
                        }
                    }
                } else {
                    match serde_json::to_vec(&msg) {
                        Ok(mut b) => {
                            let mut out = vec![0x01];
                            out.append(&mut b);
                            out
                        }
                        Err(e) => {
                            error!("Failed to serialize NetworkMessage: {}", e);
                            continue;
                        }
                    }
                };

                let version = Version::new(uuid::Uuid::new_v4().to_string());
                let update = Update::snapshot(version, payload_bytes);
                if let Err(e) = node_send.put(&topic, update).await {
                    error!("Failed to broadcast message to {}: {}", topic, e);
                    event_tx_error.send(NetworkEvent::PeerDisconnected(format!("Broadcast error: {}", e))).ok();
                }
            }
        });

        // 2. Subscription loop
        let event_tx_sub = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(topic) = sub_rx.recv().await {
                info!("[NET] Dynamically subscribing to topic: {}", topic);
                let event_tx_inner = event_tx_sub.clone();
                match node_sub.subscribe(&topic, vec![]).await {
                    Ok(rx_new) => {
                        tokio::spawn(process_gossip_stream(rx_new, event_tx_inner));
                    }
                    Err(e) => {
                        error!("Failed to subscribe to topic {}: {}", topic, e);
                    }
                }
            }
        });

        // 3. Bootstrap loop
        tokio::spawn(async move {
            while let Some(peer_id) = bootstrap_rx.recv().await {
                if let Err(e) = node_bootstrap.join_peers(GAME_TOPIC, vec![peer_id]).await {
                    error!("Failed to join peer {}: {}", peer_id, e);
                }
            }
        });

        // 4. Main gossip pump (global topic)
        process_gossip_stream(rx, event_tx_clone).await;
    });
}

/// Bind the causal identity to the verified signer.
///
/// The `agent_id` carried inside a `Move` is set by the sender and is NOT
/// authenticated on its own — a malicious peer could put a victim's identity
/// there. After a signature verifies, we overwrite `agent_id` with the public
/// key that actually signed the message (`session_pubkey`). The causal
/// fork-check in [`handle_network_events`] keys on `agent_id`, so this makes
/// that identity unforgeable: to act as identity X you must hold X's signing
/// key. Closes the impersonation gap the TLA+ model assumed away — see
/// docs/plans/causal-authentication.md (Gap A1).
fn bind_identity(mut signed: SignedNetworkMessage) -> NetworkMessage {
    if let NetworkMessage::Move { agent_id, .. } = &mut signed.msg {
        *agent_id = signed.session_pubkey.clone();
    }
    signed.msg
}

async fn process_gossip_stream(
    mut rx: iroh_gossip::api::GossipReceiver,
    event_tx: tokio::sync::mpsc::UnboundedSender<NetworkEvent>,
) {
    while let Some(result) = rx.next().await {
        match result {
            Ok(IrohEvent::NeighborUp(peer_id)) => {
                info!("GOSSIP NeighborUp: {}", peer_id);
                let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                event_tx.send(NetworkEvent::PeerConnected(bs58_id.clone())).ok();

                event_tx.send(NetworkEvent::PeerDiscovered(PeerInfo {
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
                })).ok();
            }
            Ok(IrohEvent::Received(msg)) => {
                // Helper to extract bytes from either an Update wrapper or raw content.
                let body_bytes: Option<Vec<u8>> = if let Ok(update) = serde_json::from_slice::<Update>(&msg.content) {
                    update.body.map(|b| b.to_vec())
                } else {
                    Some(msg.content.to_vec())
                };

                if let Some(body) = body_bytes {
                    if body.is_empty() {
                        continue;
                    }
                    match body[0] {
                        // Version 0x02: bincode-encoded SignedNetworkMessage
                        0x02 => {
                            if let Ok(signed) = bincode::deserialize::<SignedNetworkMessage>(&body[1..]) {
                                if signed.verify() {
                                    event_tx.send(NetworkEvent::MessageReceived(bind_identity(signed))).ok();
                                } else {
                                    let game_id = signed.msg.game_id();
                                    event_tx.send(NetworkEvent::InvalidMoveRejected {
                                        game_id,
                                        reason: "signature verification failed".to_string(),
                                    }).ok();
                                    warn!("[NET] Dropped message with invalid signature for game {}", game_id);
                                }
                            } else {
                                warn!("[NET] Failed to bincode-decode signed message");
                            }
                        }
                        // Version 0x01 or anything else: JSON fallback (legacy path)
                        _ => {
                            // Try signed JSON first
                            if let Ok(signed) = serde_json::from_slice::<SignedNetworkMessage>(&body) {
                                if signed.verify() {
                                    event_tx.send(NetworkEvent::MessageReceived(bind_identity(signed))).ok();
                                } else {
                                    let game_id = signed.msg.game_id();
                                    event_tx.send(NetworkEvent::InvalidMoveRejected {
                                        game_id,
                                        reason: "signature verification failed".to_string(),
                                    }).ok();
                                    warn!("[NET] Dropped message with invalid signature for game {}", game_id);
                                }
                            } else if let Ok(net_msg) = serde_json::from_slice::<NetworkMessage>(&body) {
                                // Plain unsigned legacy message. A3: rejected by
                                // default — accepting it would let a peer bypass
                                // authentication entirely by sending plaintext.
                                // The `allow-unsigned-p2p` feature re-enables it
                                // for local dev/testing only.
                                #[cfg(feature = "allow-unsigned-p2p")]
                                {
                                    event_tx.send(NetworkEvent::MessageReceived(net_msg)).ok();
                                }
                                #[cfg(not(feature = "allow-unsigned-p2p"))]
                                {
                                    let game_id = net_msg.game_id();
                                    event_tx.send(NetworkEvent::InvalidMoveRejected {
                                        game_id,
                                        reason: "unsigned messages are not accepted".to_string(),
                                    }).ok();
                                    warn!("[NET] Dropped unsigned message for game {}", game_id);
                                }
                            }
                        }
                    }
                }
            }
            Ok(IrohEvent::NeighborDown(peer_id)) => {
                info!("GOSSIP NeighborDown: {}", peer_id);
                let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                event_tx.send(NetworkEvent::PeerDisconnected(bs58_id)).ok();
            }
            _ => {}
        }
    }
}

/// Polling system to read NetworkEvents from the background task and write them as Bevy events.
pub fn handle_network_events(
    mut network_state: ResMut<BraidNetworkState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<ResignEvent>,
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
                // P2P-layer replay protection: reject moves/resigns with stale nonce
                let game_id = msg.game_id();
                if let NetworkMessage::Move { nonce, .. } | NetworkMessage::Resign { nonce, .. } = msg {
                    let expected = network_state.expected_nonces.get(&game_id).copied().unwrap_or(1);
                    if *nonce < expected {
                        warn!(
                            "[NET] Replayed move/resign for game {}: nonce {} < expected {}",
                            game_id, nonce, expected
                        );
                        network_events.write(NetworkEvent::InvalidMoveRejected {
                            game_id,
                            reason: format!("replay nonce {} < expected {}", nonce, expected),
                        });
                        continue;
                    }
                    // Accept and advance expected nonce
                    network_state.expected_nonces.insert(game_id, nonce.saturating_add(1));
                }

                // A2: build the per-game roster of allowed signer keys from
                // SessionInfo (broadcast only after the VPS confirms a session is
                // active). Capped at two — the two participants.
                if let NetworkMessage::SessionInfo { game_id: sg, session_pubkey, .. } = msg {
                    #[cfg(feature = "solana")]
                    let key = session_pubkey.to_bytes().to_vec();
                    #[cfg(not(feature = "solana"))]
                    let key = session_pubkey.0.to_vec();
                    let entry = causal.roster.entry(*sg).or_default();
                    if !entry.contains(&key) && entry.len() < 2 {
                        entry.push(key);
                    }
                }

                // Causal chain check (Gap 1/3): verify seq continuity + parent version.
                // Only applied when the sender populates the causal fields (non-legacy).
                if let NetworkMessage::Move {
                    turn,
                    next_fen,
                    agent_id,
                    seq,
                    parent_version,
                    ..
                } = msg
                {
                    if !agent_id.is_empty() && *seq > 0 {
                        // A2: roster check. `agent_id` here is the VERIFIED signer
                        // (bound in `bind_identity`). Once we know this game's
                        // participant session keys (from `SessionInfo`), reject any
                        // move whose signer is not one of them — a stranger cannot
                        // inject a move into a game they are not part of, even with
                        // a valid signature of their own.
                        if let Some(allowed) = causal.roster.get(&game_id) {
                            if !allowed.is_empty() && !allowed.contains(agent_id) {
                                warn!(
                                    "[NET] Move from non-participant signer for game {} (agent {:?})",
                                    game_id,
                                    &agent_id[..4.min(agent_id.len())]
                                );
                                network_events.write(NetworkEvent::InvalidMoveRejected {
                                    game_id,
                                    reason: "signer is not a participant in this game".to_string(),
                                });
                                continue;
                            }
                        }

                        let agent_key = (game_id, agent_id.clone());
                        let last = causal.last_seq.get(&agent_key).copied().unwrap_or(0);
                        if *seq != last + 1 {
                            warn!(
                                "[NET] Causal seq gap for game {} agent {:?}: got {} expected {}",
                                game_id,
                                &agent_id[..4.min(agent_id.len())],
                                seq,
                                last + 1
                            );
                            network_events.write(NetworkEvent::InvalidMoveRejected {
                                game_id,
                                reason: format!("causal seq gap: got {} expected {}", seq, last + 1),
                            });
                            continue;
                        }
                        // Equivocation guard. Once we have a head for THIS agent
                        // (the game has progressed past their first move), EVERY
                        // subsequent move must name that head as its parent —
                        // including a move that falsely claims genesis ("0") or an
                        // empty parent. Gating on `parent_version != "0"` (as
                        // before) let a malicious peer bypass the check by attaching
                        // "0" to a move with an otherwise-valid sequence number,
                        // forking our local head. Verified by the TLA+ model in
                        // specs/CausalChain.tla (CC_byzantine_current = fork,
                        // CC_byzantine_fixed = safe across 15.2M states).
                        let our_head = causal.head_version.get(&agent_key).cloned().unwrap_or_default();
                        if !our_head.is_empty() && parent_version != &our_head {
                            warn!(
                                "[NET] Equivocation detected for game {}: \
                                 sender parent_version={} our head={}",
                                game_id, parent_version, our_head
                            );
                            network_events.write(NetworkEvent::InvalidMoveRejected {
                                game_id,
                                reason: format!(
                                    "equivocation: parent {} != head {}",
                                    parent_version, our_head
                                ),
                            });
                            continue;
                        }
                        causal.last_seq.insert(agent_key.clone(), *seq);
                        // Advance THIS agent's head (Gap B: per-sender lane).
                        let new_head = braid_uri::version_hash(next_fen, *turn as u32);
                        causal.head_version.insert(agent_key, new_head);
                    } else {
                        // Legacy move without causal fields: keep a per-game head
                        // under the empty-agent key so resync still has a reference.
                        let new_head = braid_uri::version_hash(next_fen, *turn as u32);
                        causal.head_version.insert((game_id, Vec::new()), new_head);
                    }
                }

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
                    NetworkMessage::Resign { winner, .. } => {
                        resign_events.write(ResignEvent {
                            winner: winner.clone(),
                            remote: true,
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
        GameOverState::WhiteWonByResignation => (Some("white".to_string()), "resignation"),
        GameOverState::BlackWon => (Some("black".to_string()), "checkmate"),
        GameOverState::BlackWonByTime => (Some("black".to_string()), "timeout"),
        GameOverState::BlackWonByResignation => (Some("black".to_string()), "resignation"),
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

/// Convert `NetworkEvent::MessageReceived(NetworkMessage::Move)` into `NetworkMoveEvent`
/// so that `handle_network_moves` can apply opponent moves to the local board.
/// Runs for both Iroh-gossip and Braid-HTTP sourced move messages.
pub fn dispatch_remote_moves(
    mut network_events: MessageReader<NetworkEvent>,
    mut move_events: MessageWriter<crate::game::events::NetworkMoveEvent>,
    game_mode: Res<crate::core::states::GameMode>,
) {
    if *game_mode != crate::core::states::GameMode::BraidMultiplayer {
        return;
    }
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Move { move_uci, next_fen, .. }) = event {
            if move_uci.len() >= 4 {
                let bytes = move_uci.as_bytes();
                let from_file = bytes[0].wrapping_sub(b'a');
                let from_rank = bytes[1].wrapping_sub(b'1');
                let to_file = bytes[2].wrapping_sub(b'a');
                let to_rank = bytes[3].wrapping_sub(b'1');
                let promotion = move_uci.get(4..5).and_then(|s| s.chars().next());
                info!("[NET] Dispatching remote move {} as NetworkMoveEvent", move_uci);
                move_events.write(crate::game::events::NetworkMoveEvent {
                    from: (from_file, from_rank),
                    to: (to_file, to_rank),
                    promotion,
                    expected_fen: Some(next_fen.clone()),
                });
            } else {
                warn!("[NET] Received malformed UCI move: {:?}", move_uci);
            }
        }
    }
}

/// React to [`NetworkMessage::ResyncResponse`] by overwriting the local engine with the
/// authoritative FEN sent by the opponent.  Separate system so it doesn't need to share
/// the `network_events` MessageReader cursor with `dispatch_remote_moves`.
pub fn handle_resync_response(
    mut network_events: MessageReader<NetworkEvent>,
    mut engine: ResMut<crate::engine::board_state::ChessEngine>,
    mut selection: ResMut<crate::game::resources::Selection>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::ResyncResponse {
            committed_fen,
            ..
        }) = event
        {
            warn!("[NET] Applying ResyncResponse — overwriting local engine with FEN: {}", committed_fen);
            let _ = engine.set_from_fen(committed_fen);
            *selection = crate::game::resources::Selection::default();
        }
    }
}

/// Route inbound draw, timeout, rematch, pause/resume, and ping/pong messages into Bevy events.
pub fn handle_game_control_messages(
    mut network_events: MessageReader<NetworkEvent>,
    mut draw_offer: MessageWriter<crate::game::events::DrawOfferEvent>,
    mut draw_response: MessageWriter<crate::game::events::DrawResponseEvent>,
    mut rematch_offer: MessageWriter<crate::game::events::RematchOfferEvent>,
    mut rematch_response: MessageWriter<crate::game::events::RematchResponseEvent>,
    mut flag_timeout: MessageWriter<crate::game::events::FlagTimeoutEvent>,
    network_state: Res<BraidNetworkState>,
    mut game_timer: Option<ResMut<crate::game::resources::GameTimer>>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    for event in network_events.read() {
        let NetworkEvent::MessageReceived(msg) = event else { continue };

        match msg {
            NetworkMessage::DrawOffer { player, .. } => {
                draw_offer.write(crate::game::events::DrawOfferEvent {
                    player: player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::DrawResponse { player, accepted, .. } => {
                draw_response.write(crate::game::events::DrawResponseEvent {
                    player: player.clone(),
                    accepted: *accepted,
                    remote: true,
                });
            }
            NetworkMessage::FlagTimeout { flagged_player, .. } => {
                flag_timeout.write(crate::game::events::FlagTimeoutEvent {
                    flagged_player: flagged_player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::RematchOffer { player, .. } => {
                rematch_offer.write(crate::game::events::RematchOfferEvent {
                    player: player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::RematchResponse { player, accepted, .. } => {
                rematch_response.write(crate::game::events::RematchResponseEvent {
                    player: player.clone(),
                    accepted: *accepted,
                    remote: true,
                });
            }
            NetworkMessage::PauseRequest { .. } => {
                if let Some(ref mut timer) = game_timer {
                    timer.is_running = false;
                    info!("[NET] Clocks paused by remote player");
                }
            }
            NetworkMessage::ResumeRequest { .. } => {
                if let Some(ref mut timer) = game_timer {
                    timer.is_running = true;
                    info!("[NET] Clocks resumed by remote player");
                }
            }
            NetworkMessage::Ping { game_id, timestamp_ms } => {
                // Reply with Pong immediately.
                if let Some(tx) = &network_state.message_sender {
                    let _ = tx.send(NetworkMessage::Pong {
                        game_id: *game_id,
                        timestamp_ms: *timestamp_ms,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Forward local draw offers, draw responses, rematch messages, and flag timeouts to the network.
pub fn send_local_draw_events(
    mut local_draw_offers: MessageReader<crate::game::events::DrawOfferEvent>,
    mut local_draw_responses: MessageReader<crate::game::events::DrawResponseEvent>,
    mut local_rematch_offers: MessageReader<crate::game::events::RematchOfferEvent>,
    mut local_rematch_responses: MessageReader<crate::game::events::RematchResponseEvent>,
    mut local_flag_timeouts: MessageReader<crate::game::events::FlagTimeoutEvent>,
    network_state: Res<BraidNetworkState>,
    session: Option<Res<crate::multiplayer::network::braid_pvp::BraidPvpSession>>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    let game_id = session
        .as_ref()
        .and_then(|s| s.game_id.parse::<u64>().ok())
        .unwrap_or(0);
    let Some(tx) = &network_state.message_sender else { return };

    for ev in local_draw_offers.read() {
        if ev.remote { continue; }
        let _ = tx.send(NetworkMessage::DrawOffer { game_id, player: ev.player.clone() });
    }
    for ev in local_draw_responses.read() {
        if ev.remote { continue; }
        let _ = tx.send(NetworkMessage::DrawResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_rematch_offers.read() {
        if ev.remote { continue; }
        let _ = tx.send(NetworkMessage::RematchOffer { game_id, player: ev.player.clone() });
    }
    for ev in local_rematch_responses.read() {
        if ev.remote { continue; }
        let _ = tx.send(NetworkMessage::RematchResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_flag_timeouts.read() {
        if ev.remote { continue; }
        let _ = tx.send(NetworkMessage::FlagTimeout {
            game_id,
            flagged_player: ev.flagged_player.clone(),
        });
    }
}

/// When the opponent sends [`NetworkMessage::ResyncRequest`], reply with our current engine FEN
/// as a [`NetworkMessage::ResyncResponse`] so they can snap back to the correct board state.
pub fn handle_resync_request(
    mut network_events: MessageReader<NetworkEvent>,
    engine: Res<crate::engine::board_state::ChessEngine>,
    network_state: Res<BraidNetworkState>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::ResyncRequest { game_id }) = event {
            if let Some(tx) = &network_state.message_sender {
                let response = NetworkMessage::ResyncResponse {
                    game_id: *game_id,
                    committed_fen: engine.current_fen().to_string(),
                    committed_turn: 0,
                };
                if let Err(e) = tx.send(response) {
                    warn!("[NET] Failed to send ResyncResponse: {e}");
                } else {
                    info!("[NET] Sent ResyncResponse for game {game_id}");
                }
            }
        }
    }
}

/// Send a [`NetworkMessage::Ping`] on a fixed interval while a game session is active.
pub fn tick_heartbeat(
    time: Res<Time>,
    mut heartbeat: ResMut<HeartbeatState>,
    network_state: Res<BraidNetworkState>,
    session: Option<Res<crate::multiplayer::network::braid_pvp::BraidPvpSession>>,
    game_mode: Res<crate::core::states::GameMode>,
    mut game_over: ResMut<crate::game::resources::history::game_over::GameOverState>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
) {
    use crate::core::states::GameMode;
    use crate::multiplayer::network::protocol::NetworkMessage;

    if *game_mode != GameMode::BraidMultiplayer {
        return;
    }
    if heartbeat.timed_out {
        return;
    }

    let dt = time.delta_secs();
    heartbeat.since_last_ping += dt;
    heartbeat.since_last_pong += dt;

    // Send a ping on interval (±0.5 s jitter to spread VPS load across clients).
    if heartbeat.since_last_ping >= heartbeat.ping_interval {
        // Jitter: next interval = 4.5 .. 5.5 s based on system-time sub-millis
        let jitter = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis() % 1000) as f32 / 1000.0 - 0.5;
        heartbeat.ping_interval = 5.0 + jitter;
        heartbeat.since_last_ping = 0.0;
        if let (Some(tx), Some(sess)) = (&network_state.message_sender, session.as_ref()) {
            let game_id = sess.game_id.parse::<u64>().unwrap_or(0);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let _ = tx.send(NetworkMessage::Ping { game_id, timestamp_ms: ts });
        }
    }

    // Declare disconnect if pong is overdue.
    if heartbeat.since_last_pong >= heartbeat.timeout_secs {
        heartbeat.timed_out = true;
        warn!("[NET] Heartbeat timeout — opponent disconnected after {:.0}s silence", heartbeat.since_last_pong);
        // Treat as a win for the local player (opponent abandoned).
        // The game-over screen will show the appropriate message.
        *game_over = crate::game::resources::history::game_over::GameOverState::Stalemate;
        next_state.set(crate::core::GameState::MainMenu);
    }
}

/// Reset heartbeat counters when a Pong arrives.
pub fn handle_pong(
    mut network_events: MessageReader<NetworkEvent>,
    mut heartbeat: ResMut<HeartbeatState>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(
            crate::multiplayer::network::protocol::NetworkMessage::Pong { .. }
        ) = event {
            heartbeat.since_last_pong = 0.0;
        }
    }
}

pub fn load_or_generate_key() -> (SecretKey, [u8; 32]) {
    // Derive a stable key from the wallet pubkey when Tauri sets XFCHESS_WALLET_PUBKEY.
    // Same wallet → same NodeID across every session.
    if let Ok(pubkey_str) = std::env::var("XFCHESS_WALLET_PUBKEY") {
        if !pubkey_str.is_empty() {
            let hash = Sha256::digest(pubkey_str.as_bytes());
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&hash);
            let sk = SecretKey::from_bytes(&arr);
            info!("[NET] NodeID derived from wallet pubkey (stable)");
            return (sk, arr);
        }
    }

    // Fall back to XFCHESS_IDENTITY env file, then the persistent config-dir key.
    if let Ok(env_path) = std::env::var("XFCHESS_IDENTITY") {
        let key_file = PathBuf::from(env_path);
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
        return (sk, bytes);
    }

    // Stable persistent identity: survives restarts and wallet rotations.
    let sk = crate::multiplayer::network::identity::load_or_create();
    let bytes = sk.to_bytes();
    (sk, bytes)
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    /// A1 regression: an attacker who signs with their OWN key but stuffs a
    /// victim's identity into `agent_id` must not be able to act as the victim.
    /// `bind_identity` discards the claimed `agent_id` and substitutes the
    /// verified signer, so the causal/roster checks see the attacker's real key.
    #[test]
    fn bind_identity_uses_verified_signer_not_claimed_agent_id() {
        let attacker_sk = [9u8; 32];
        let victim_id = vec![1u8; 32];

        let msg = NetworkMessage::Move {
            game_id: 1,
            turn: 1,
            move_uci: "e2e4".to_string(),
            next_fen: "f".to_string(),
            nonce: 1,
            timestamp_ms: 0,
            agent_id: victim_id.clone(), // forged: claims to be the victim
            seq: 1,
            parent_version: "0".to_string(),
        };

        let signed = SignedNetworkMessage::sign(msg, &attacker_sk);
        // The signature IS valid — for the attacker's own key.
        assert!(signed.verify());
        let attacker_pub = signed.session_pubkey.clone();

        let bound = bind_identity(signed);
        match bound {
            NetworkMessage::Move { agent_id, .. } => {
                assert_eq!(agent_id, attacker_pub, "agent_id must be the verified signer");
                assert_ne!(agent_id, victim_id, "the forged victim identity must be discarded");
            }
            _ => panic!("expected Move"),
        }
    }

    /// Non-Move messages are passed through unchanged by `bind_identity`.
    #[test]
    fn bind_identity_leaves_non_move_untouched() {
        let sk = [7u8; 32];
        let msg = NetworkMessage::ResyncRequest { game_id: 42 };
        let signed = SignedNetworkMessage::sign(msg, &sk);
        assert!(matches!(bind_identity(signed), NetworkMessage::ResyncRequest { game_id: 42 }));
    }
}
