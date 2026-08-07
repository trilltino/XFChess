use bevy::prelude::*;
use braid_chess;
use braid_core::{Update, Version};
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig};
use futures_lite::StreamExt;
use iroh::{EndpointId, SecretKey};
use iroh_gossip::api::Event as IrohEvent;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use crate::game::events::ResignEvent;
use crate::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};
use crate::multiplayer::network::reorder::IngestOutcome;
use crate::multiplayer::types::*;
use crate::multiplayer::TokioRuntime;

/// Peers and topics the Iroh node task currently knows about.
///
/// Exists because an iroh-gossip topic is a *separate* swarm per `TopicId`:
/// `Gossip::subscribe(topic, bootstrap)` only dials the peers in `bootstrap`,
/// and being neighbours on one topic gives you nothing on another. Moves are
/// broadcast on the per-game topic `GAME_TOPIC/<game_id>`, but the only
/// `join_peers` call this code ever made was against the *global* `GAME_TOPIC`
/// — so the per-game swarm had exactly one member on each side and gossip
/// never delivered a single move between clients.
///
/// Both orderings have to work, since "we learned the peer" and "we joined the
/// game topic" race with no guaranteed winner:
/// - peer known first  → [`GossipMesh::subscribe`] passes it as bootstrap.
/// - topic joined first → [`GossipMesh::add_peer`] back-fills it into every
///   topic already subscribed.
#[derive(Default)]
struct GossipMesh {
    peers: std::collections::HashSet<EndpointId>,
    topics: std::collections::HashSet<String>,
}

impl GossipMesh {
    /// Record a topic and return the peers to bootstrap it with.
    fn subscribe(&mut self, topic: &str) -> Vec<EndpointId> {
        self.topics.insert(topic.to_string());
        self.peers.iter().copied().collect()
    }

    /// Record a peer and return every topic it must be joined into.
    fn add_peer(&mut self, peer: EndpointId) -> Vec<String> {
        self.peers.insert(peer);
        self.topics.iter().cloned().collect()
    }
}

#[cfg(feature = "solana")]
use crate::game::events::{GameEndedEvent, MoveMadeEvent};
use crate::game::resources::history::game_over::GameOverState;
#[cfg(feature = "solana")]
use crate::rendering::PieceType;

pub const GAME_TOPIC: &str = "/xfchess-game";

/// Initializes the Braid/Iroh networking layer in a background Tokio task.
pub fn initialize_braid_network(
    mut network_state: ResMut<OnlineNetworkState>,
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
    network_state.event_sender = Some(event_tx.clone()); // relay bridge injects incoming here
    network_state.message_sender = Some(msg_tx);
    network_state.bootstrap_sender = Some(bootstrap_tx);
    network_state.subscription_sender = Some(sub_tx);

    // Cloned `Arc`, not a snapshot — this task reads the *shared* cell fresh
    // on every send (see `OnlineNetworkState::session_signing_key_shared`'s
    // doc comment), since a plain copy taken here would forever be `None`:
    // this function runs at app boot, long before any wallet connects and a
    // real signing key exists.
    let session_signing_key_shared = network_state.session_signing_key_shared.clone();
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

        // Namespaced by XFCHESS_WALLET_PORT (same instance-scoping signal used
        // for the hot wallet path, session-key storage, and wallet bridge
        // elsewhere) so two same-machine instances (`just dev2`'s P1/P2, base
        // port 7454 vs 7464) don't both bind 127.0.0.1:8181 — the second one
        // always failed with "Only one usage of each socket address ...
        // (os error 10048)" and silently never got a spectator TCP bridge.
        let wallet_port: u16 = std::env::var("XFCHESS_WALLET_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7454);
        let proxy_port = 8181u16.wrapping_add(wallet_port.wrapping_sub(7454));
        let listen_addr = format!("127.0.0.1:{proxy_port}")
            .parse()
            .expect("valid socket addr");

        let config = BraidIrohConfig {
            secret_key: Some(secret_key),
            discovery: DiscoveryConfig::Real,
            proxy_config: Some(braid_iroh::ProxyConfig {
                listen_addr,
                default_peer: derived_node_id,
            }),
            data_dir: braid_data_dir,
        };

        let node = match BraidIrohNode::spawn(config).await {
            Ok(n) => n,
            Err(e) => {
                error!("❌ Failed to spawn BraidIrohNode: {}", e);
                event_tx_clone
                    .send(NetworkEvent::PeerDisconnected(format!(
                        "Spawn failed: {}",
                        e
                    )))
                    .ok();
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

        // Shared between the subscription and bootstrap loops so a peer and a
        // topic learned in either order still end up joined — see `GossipMesh`.
        let mesh = std::sync::Arc::new(tokio::sync::Mutex::new(GossipMesh::default()));
        mesh.lock().await.topics.insert(GAME_TOPIC.to_string());

        let node_arc = std::sync::Arc::new(node);
        let node_send = node_arc.clone();
        let node_bootstrap = node_arc.clone();
        let node_sub = node_arc.clone();
        let mesh_sub = mesh.clone();
        let mesh_bootstrap = mesh.clone();

        // 1. Outgoing message loop
        let event_tx_error = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                // Determine topic before msg is potentially consumed by signing.
                let topic = match &msg {
                    NetworkMessage::GameInvite { .. }
                    | NetworkMessage::InviteResponse { .. }
                    | NetworkMessage::GameStart { .. } => GAME_TOPIC.to_string(),
                    _ => format!("{}/{}", GAME_TOPIC, msg.game_id()),
                };
                // Captured before `msg` is potentially consumed below — kept
                // deliberately cheap (variant name + game_id only) so this
                // log line is safe to leave on permanently, unlike dumping
                // full message contents.
                let msg_kind = msg.kind_str();
                let msg_game_id = msg.game_id();

                // Read fresh every send — see `session_signing_key_shared`'s
                // doc comment for why a one-time snapshot doesn't work here.
                let session_signing_key = session_signing_key_shared
                    .read()
                    .map(|guard| *guard)
                    .unwrap_or(None);
                let signed = session_signing_key.is_some();

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
                    error!(
                        "[NET] Broadcast FAILED: {} for game {} via gossip ({}): {}",
                        msg_kind,
                        msg_game_id,
                        if signed { "signed" } else { "UNSIGNED" },
                        e
                    );
                    event_tx_error
                        .send(NetworkEvent::PeerDisconnected(format!(
                            "Broadcast error: {}",
                            e
                        )))
                        .ok();
                } else {
                    info!(
                        "[NET] Sent {} for game {} via gossip ({})",
                        msg_kind,
                        msg_game_id,
                        if signed {
                            "signed"
                        } else {
                            "UNSIGNED — will be dropped by any peer without allow-unsigned-p2p"
                        }
                    );
                }
            }
        });

        // 2. Subscription loop
        let event_tx_sub = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(topic) = sub_rx.recv().await {
                // Bootstrap with every peer we already know. Subscribing with
                // an empty bootstrap list (as this did) joins a swarm of one:
                // the topic exists locally, broadcasts succeed, and nothing is
                // ever delivered to the opponent.
                //
                // The lock is held across the subscribe so the bootstrap loop
                // can't observe a topic that `SubscriptionManager` hasn't
                // registered yet (its `join_peers` errors on an unknown topic)
                // nor add a peer that this snapshot has already missed.
                let mut mesh = mesh_sub.lock().await;
                let bootstrap = mesh.subscribe(&topic);
                info!(
                    "[NET] Dynamically subscribing to topic: {} ({} bootstrap peer(s))",
                    topic,
                    bootstrap.len()
                );
                let event_tx_inner = event_tx_sub.clone();
                match node_sub.subscribe(&topic, bootstrap).await {
                    Ok(rx_new) => {
                        tokio::spawn(process_gossip_stream(rx_new, event_tx_inner));
                    }
                    Err(e) => {
                        error!("Failed to subscribe to topic {}: {}", topic, e);
                        mesh.topics.remove(&topic);
                    }
                }
                drop(mesh);
            }
        });

        // 3. Bootstrap loop
        tokio::spawn(async move {
            while let Some(peer_id) = bootstrap_rx.recv().await {
                // Join the peer into every topic we hold, not just the global
                // one — the per-game move topic is the one that matters, and
                // it may have been subscribed before this peer was known.
                let topics = mesh_bootstrap.lock().await.add_peer(peer_id);
                for topic in topics {
                    if let Err(e) = node_bootstrap.join_peers(&topic, vec![peer_id]).await {
                        error!("Failed to join peer {} to topic {}: {}", peer_id, topic, e);
                    } else {
                        info!("[NET] Joined peer {} into topic {}", peer_id, topic);
                    }
                }
            }
        });

        // 4. Main gossip pump (global topic)
        process_gossip_stream(rx, event_tx_clone).await;
    });
}

/// Bind the causal identity to the verified signer.
///
/// The `signer_pubkey` carried inside a `Move` is set by the sender and is NOT
/// authenticated on its own — a malicious peer could put a victim's identity
/// there. After a signature verifies, we overwrite `signer_pubkey` with the public
/// key that actually signed the message (`session_pubkey`). The causal
/// fork-check in [`handle_network_events`] keys on `signer_pubkey`, so this makes
/// that identity unforgeable: to act as identity X you must hold X's signing
/// key. Closes the impersonation gap the TLA+ model assumed away — see
/// docs/plans/causal-authentication.md (Gap A1).
fn bind_identity(mut signed: SignedNetworkMessage) -> NetworkMessage {
    if let NetworkMessage::Move { signer_pubkey, .. } = &mut signed.msg {
        *signer_pubkey = signed.session_pubkey.clone();
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
                info!("[NET] Peer connected via gossip: {}", peer_id);
                let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                event_tx
                    .send(NetworkEvent::PeerConnected(bs58_id.clone()))
                    .ok();

                event_tx
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
                // Helper to extract bytes from either an Update wrapper or raw content.
                let body_bytes: Option<Vec<u8>> =
                    if let Ok(update) = serde_json::from_slice::<Update>(&msg.content) {
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
                            if let Ok(signed) =
                                bincode::deserialize::<SignedNetworkMessage>(&body[1..])
                            {
                                if signed.verify() {
                                    info!(
                                        "[NET] Received {} for game {} via gossip (signed, verified)",
                                        signed.msg.kind_str(),
                                        signed.msg.game_id()
                                    );
                                    event_tx
                                        .send(NetworkEvent::MessageReceived(bind_identity(signed)))
                                        .ok();
                                } else {
                                    let game_id = signed.msg.game_id();
                                    event_tx
                                        .send(NetworkEvent::InvalidMoveRejected {
                                            game_id,
                                            reason: "signature verification failed".to_string(),
                                        })
                                        .ok();
                                    warn!(
                                        "[NET] Dropped message with invalid signature for game {}",
                                        game_id
                                    );
                                }
                            } else {
                                warn!("[NET] Failed to bincode-decode signed message");
                            }
                        }
                        // Version 0x01 or anything else: JSON fallback (legacy path)
                        _ => {
                            // Try signed JSON first
                            if let Ok(signed) =
                                serde_json::from_slice::<SignedNetworkMessage>(&body)
                            {
                                if signed.verify() {
                                    info!(
                                        "[NET] Received {} for game {} via gossip (JSON, signed, verified)",
                                        signed.msg.kind_str(),
                                        signed.msg.game_id()
                                    );
                                    event_tx
                                        .send(NetworkEvent::MessageReceived(bind_identity(signed)))
                                        .ok();
                                } else {
                                    let game_id = signed.msg.game_id();
                                    event_tx
                                        .send(NetworkEvent::InvalidMoveRejected {
                                            game_id,
                                            reason: "signature verification failed".to_string(),
                                        })
                                        .ok();
                                    warn!(
                                        "[NET] Dropped message with invalid signature for game {}",
                                        game_id
                                    );
                                }
                            } else if let Ok(net_msg) =
                                serde_json::from_slice::<NetworkMessage>(&body)
                            {
                                // Plain unsigned legacy message. A3: rejected by
                                // default — accepting it would let a peer bypass
                                // authentication entirely by sending plaintext.
                                // The `allow-unsigned-p2p` feature re-enables it
                                // for local dev/testing only.
                                #[cfg(feature = "allow-unsigned-p2p")]
                                {
                                    warn!(
                                        "[NET] Received {} for game {} via gossip (UNSIGNED, accepted only because allow-unsigned-p2p is enabled)",
                                        net_msg.kind_str(),
                                        net_msg.game_id()
                                    );
                                    event_tx.send(NetworkEvent::MessageReceived(net_msg)).ok();
                                }
                                #[cfg(not(feature = "allow-unsigned-p2p"))]
                                {
                                    let game_id = net_msg.game_id();
                                    event_tx
                                        .send(NetworkEvent::InvalidMoveRejected {
                                            game_id,
                                            reason: "unsigned messages are not accepted"
                                                .to_string(),
                                        })
                                        .ok();
                                    warn!(
                                        "[NET] Dropped UNSIGNED {} for game {} — sender has no session_signing_key yet (see sync_session_key_to_network)",
                                        net_msg.kind_str(),
                                        game_id
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Ok(IrohEvent::NeighborDown(peer_id)) => {
                info!("[NET] Peer disconnected from gossip: {}", peer_id);
                let bs58_id = bs58::encode(peer_id.as_bytes()).into_string();
                event_tx.send(NetworkEvent::PeerDisconnected(bs58_id)).ok();
            }
            _ => {}
        }
    }
}

/// Polling system to read NetworkEvents from the background task and write them as Bevy events.
pub fn handle_network_events(
    mut network_state: ResMut<OnlineNetworkState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
    mut pending: ResMut<crate::multiplayer::types::PendingMoveBuffer>,
    mut network_events: MessageWriter<NetworkEvent>,
    mut resign_events: MessageWriter<ResignEvent>,
) {
    let mut incoming: VecDeque<NetworkEvent> = {
        if let Some(ref mut receiver) = network_state.event_receiver {
            let mut buf = VecDeque::new();
            while let Ok(ev) = receiver.try_recv() {
                buf.push_back(ev);
            }
            buf
        } else {
            VecDeque::new()
        }
    };

    while let Some(event) = incoming.pop_front() {
        // Dual-transport reorder gate for `Move` — see
        // `crate::multiplayer::network::reorder` for why this exists: gossip
        // and the VPS relay have independent latency, so move N+1 can arrive
        // before move N. Buffer out-of-order arrivals via `NonceSequencer`
        // and release them in strict order instead of letting a fast-path
        // N+1 jump the old `expected_nonces` counter forward and permanently
        // orphan N when it later shows up (rejected as a "replay" it never
        // was). Gated arrivals land in `ready` — a separate queue from
        // `incoming` — so a message released from the buffer here is never
        // re-fed through the gate a second time (it would look like a
        // duplicate, since the sequencer's `expected` has already moved past
        // it).
        //
        // `Resign` deliberately does NOT go through this gate: it's a
        // terminal, idempotent, order-independent signal, not part of the
        // ordered move stream — sharing this gate previously meant every
        // resign (which used a hardcoded `nonce: 0`, always `< expected`
        // once any real move had been seen) was silently dropped and never
        // reached the opponent.
        let mut ready: VecDeque<NetworkEvent> = VecDeque::new();
        if let NetworkEvent::MessageReceived(NetworkMessage::Move { nonce, .. }) = &event {
            let nonce = *nonce;
            let (game_id, msg) = match &event {
                NetworkEvent::MessageReceived(msg) => (msg.game_id(), msg.clone()),
                _ => unreachable!(),
            };

            let outcome = pending
                .sequencers
                .entry(game_id)
                .or_insert_with(|| {
                    crate::multiplayer::network::reorder::NonceSequencer::new(
                        crate::multiplayer::types::PendingMoveBuffer::MAX_BUFFERED,
                    )
                })
                .ingest(nonce, msg);

            match outcome {
                IngestOutcome::Duplicate => {
                    warn!(
                        "[NET] Replayed move for game {}: nonce {} already applied",
                        game_id, nonce
                    );
                    network_events.write(NetworkEvent::InvalidMoveRejected {
                        game_id,
                        reason: format!("replay nonce {}", nonce),
                    });
                    continue;
                }
                IngestOutcome::Ready(batch) if batch.is_empty() => {
                    pending
                        .oldest_buffered_since
                        .entry(game_id)
                        .or_insert_with(Instant::now);
                    info!(
                        "[NET] Move nonce {} for game {} arrived out of order — buffered pending earlier nonce",
                        nonce, game_id
                    );
                    continue;
                }
                IngestOutcome::Ready(batch) => {
                    pending.oldest_buffered_since.remove(&game_id);
                    for msg in batch {
                        ready.push_back(NetworkEvent::MessageReceived(msg));
                    }
                }
                IngestOutcome::Overflow { resync_from } => {
                    pending.oldest_buffered_since.remove(&game_id);
                    warn!(
                        "[NET] Gap at nonce {} for game {} never filled — forcing resync",
                        resync_from, game_id
                    );
                    if let Some(tx) = &network_state.message_sender {
                        let _ = tx.send(NetworkMessage::ResyncRequest { game_id });
                    }
                    network_events.write(NetworkEvent::InvalidMoveRejected {
                        game_id,
                        reason: format!(
                            "gap at nonce {} unrecoverable — resync requested",
                            resync_from
                        ),
                    });
                    continue;
                }
            }
        } else {
            ready.push_back(event);
        }

        while let Some(event) = ready.pop_front() {
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
                    // Move's own nonce ordering/replay protection is handled by
                    // `NonceSequencer` before we get here (see above) — by this
                    // point a `Move` has already been confirmed in-order.
                    let game_id = msg.game_id();

                    // A2: build the per-game roster of allowed signer keys from
                    // SessionInfo (broadcast only after the VPS confirms a session is
                    // active). Capped at two — the two participants.
                    //
                    // Must use `signing_pubkey`, NOT `session_pubkey` — the roster is
                    // checked against `signer_pubkey`, which `bind_identity` sets from the
                    // verified signer of this message's own P2P envelope (i.e. the
                    // sender's `session_signing_key`, an ephemeral per-connection
                    // gossip key). `session_pubkey` is a completely different key
                    // (the VPS/backend on-chain session-delegation key). Populating
                    // the roster from it meant no real move's signer could ever
                    // match, so every move was rejected as "non-participant" as soon
                    // as the roster had any entry at all.
                    if let NetworkMessage::SessionInfo {
                        game_id: sg,
                        player_pubkey,
                        signing_pubkey,
                        ..
                    } = msg
                    {
                        // Phase C (`docs/plans/networking-hardening-plan.md`):
                        // once the backend-verified on-chain wallets are
                        // known for this game, a claim whose `player_pubkey`
                        // isn't actually white or black is never trusted
                        // into the roster — closing the race a forged
                        // SessionInfo could otherwise win by arriving first.
                        // Absent (casual game, or fetch not complete yet)
                        // falls back to the original trust-first bootstrap.
                        let claim_trusted = match causal.verified_wallets.get(sg) {
                            Some((white, black)) => {
                                let claimed = player_pubkey.to_string();
                                &claimed == white || &claimed == black
                            }
                            None => true,
                        };

                        if claim_trusted {
                            #[cfg(feature = "solana")]
                            let key = signing_pubkey.to_bytes().to_vec();
                            #[cfg(not(feature = "solana"))]
                            let key = signing_pubkey.0.to_vec();
                            let entry = causal.roster.entry(*sg).or_default();
                            if !entry.contains(&key) && entry.len() < 2 {
                                let key_prefix = key[..4.min(key.len())].to_vec();
                                entry.push(key);
                                info!(
                                    "[NET] Roster for game {} now has {} entry(ies) — added signer {:?}",
                                    sg,
                                    entry.len(),
                                    key_prefix
                                );
                            }
                        } else {
                            warn!(
                                "[NET] Rejected SessionInfo claim for game {} — player_pubkey {} does not match on-chain white/black",
                                sg, player_pubkey
                            );
                        }
                    }

                    // Causal chain check (Gap 1/3): verify seq continuity + parent version.
                    // Only applied when the sender populates the causal fields (non-legacy).
                    if let NetworkMessage::Move {
                        turn,
                        next_fen,
                        signer_pubkey,
                        seq,
                        parent_version,
                        ..
                    } = msg
                    {
                        if !signer_pubkey.is_empty() && *seq > 0 {
                            // A2: roster check. `signer_pubkey` here is the VERIFIED signer
                            // (bound in `bind_identity`). Once we know this game's
                            // participant session keys (from `SessionInfo`), reject any
                            // move whose signer is not one of them — a stranger cannot
                            // inject a move into a game they are not part of, even with
                            // a valid signature of their own.
                            if let Some(allowed) = causal.roster.get(&game_id) {
                                if !allowed.is_empty() && !allowed.contains(signer_pubkey) {
                                    warn!(
                                        "[NET] REJECTED move for game {} — signer {:?} not in roster ({} entries: {:?})",
                                        game_id,
                                        &signer_pubkey[..4.min(signer_pubkey.len())],
                                        allowed.len(),
                                        allowed
                                            .iter()
                                            .map(|k| k[..4.min(k.len())].to_vec())
                                            .collect::<Vec<_>>()
                                    );
                                    network_events.write(NetworkEvent::InvalidMoveRejected {
                                        game_id,
                                        reason: "signer is not a participant in this game"
                                            .to_string(),
                                    });
                                    continue;
                                }
                                info!(
                                    "[NET] Roster check passed for game {} — signer {:?} matched ({} entries on roster)",
                                    game_id,
                                    &signer_pubkey[..4.min(signer_pubkey.len())],
                                    allowed.len()
                                );
                            }

                            let agent_key = (game_id, signer_pubkey.clone());
                            let last = causal.last_seq.get(&agent_key).copied().unwrap_or(0);
                            if *seq != last + 1 {
                                warn!(
                                    "[NET] Causal seq gap for game {} agent {:?}: got {} expected {}",
                                    game_id,
                                    &signer_pubkey[..4.min(signer_pubkey.len())],
                                    seq,
                                    last + 1
                                );
                                network_events.write(NetworkEvent::InvalidMoveRejected {
                                    game_id,
                                    reason: format!(
                                        "causal seq gap: got {} expected {}",
                                        seq,
                                        last + 1
                                    ),
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
                            let our_head = causal
                                .head_version
                                .get(&agent_key)
                                .cloned()
                                .unwrap_or_default();
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
                            let new_head = braid_chess::version_hash(next_fen, *turn as u32);
                            let first_time_seen = causal
                                .applied_versions
                                .entry(game_id)
                                .or_default()
                                .insert(new_head.clone());
                            causal.head_version.insert(agent_key, new_head);
                            // Cross-transport dedup: if the Braid moves log
                            // already delivered this exact move (same
                            // resulting position), skip re-dispatching it to
                            // the board — see `CausalChainState::applied_versions`.
                            if !first_time_seen {
                                info!(
                                    "[NET] Move for game {} already applied via another transport — skipping duplicate dispatch",
                                    game_id
                                );
                                continue;
                            }
                        } else {
                            // Legacy move without causal fields: keep a per-game head
                            // under the empty-agent key so resync still has a reference.
                            let new_head = braid_chess::version_hash(next_fen, *turn as u32);
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
}

/// Reaps a nonce gap that a bounded `NonceSequencer` can't catch on its own:
/// a genuinely dropped move (not just reordered) that the mover's own client
/// has nothing to follow up with, since it's still waiting for the opponent
/// to see that very move — the buffer never grows past one entry, so
/// `NonceSequencer`'s count bound never fires. Runs every frame; only games
/// with a buffered entry older than `PendingMoveBuffer::STALE_AFTER` pay any
/// cost.
pub fn sweep_stale_move_buffers(
    mut pending: ResMut<crate::multiplayer::types::PendingMoveBuffer>,
    network_state: Res<OnlineNetworkState>,
    mut network_events: MessageWriter<NetworkEvent>,
) {
    let stale: Vec<u64> = pending
        .oldest_buffered_since
        .iter()
        .filter(|(_, since)| {
            since.elapsed() > crate::multiplayer::types::PendingMoveBuffer::STALE_AFTER
        })
        .map(|(game_id, _)| *game_id)
        .collect();

    for game_id in stale {
        pending.oldest_buffered_since.remove(&game_id);
        let Some(seq) = pending.sequencers.get_mut(&game_id) else {
            continue;
        };
        let IngestOutcome::Overflow { resync_from } = seq.expire() else {
            continue;
        };
        warn!(
            "[NET] Move buffer for game {} stale for {:?} at nonce {} — forcing resync",
            game_id,
            crate::multiplayer::types::PendingMoveBuffer::STALE_AFTER,
            resync_from
        );
        if let Some(tx) = &network_state.message_sender {
            let _ = tx.send(NetworkMessage::ResyncRequest { game_id });
        }
        network_events.write(NetworkEvent::InvalidMoveRejected {
            game_id,
            reason: format!("gap at nonce {} stale — resync requested", resync_from),
        });
    }
}

#[cfg(feature = "solana")]
pub fn feed_local_moves_to_rollup(
    mut move_events: MessageReader<MoveMadeEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    network_state: Res<OnlineNetworkState>,
) {
    if network_state.active_session.is_none() {
        return;
    }

    for event in move_events.read() {
        if event.remote {
            continue;
        }

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
    network_state: Res<OnlineNetworkState>,
) {
    if network_state.active_session.is_none() {
        return;
    }
    if !rollup_manager.is_creator {
        return;
    }

    for event in remote_events.read() {
        rollup_manager.add_local_move(event.uci.clone(), event.next_fen.clone());
    }
}

#[cfg(feature = "solana")]
pub fn handle_session_info_from_network(
    mut network_events: MessageReader<NetworkEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut session_key_manager: ResMut<
        crate::multiplayer::rollup::session_keys::HandshakeOrderingKeyManager,
    >,
    mut solana_state: Option<
        ResMut<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
            game_id,
            player_pubkey,
            session_pubkey,
            ..
        }) = event
        {
            if game_id != &rollup_manager.game_id {
                continue;
            }
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
    network_state: Res<OnlineNetworkState>,
) {
    for _event in game_end_events.read() {
        if network_state.active_session.is_none() {
            continue;
        }
        if let Some((moves, next_fens)) = rollup_manager.force_flush() {
            rollup_events.write(
                crate::multiplayer::rollup::manager::RollupEvent::GameEndBatch {
                    game_id: rollup_manager.game_id,
                    moves,
                    next_fens,
                },
            );
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
    if *emitted {
        return;
    }
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
        GameOverState::WhiteWonByAbandonment => (Some("white".to_string()), "abandonment"),
        GameOverState::BlackWonByAbandonment => (Some("black".to_string()), "abandonment"),
        GameOverState::Stalemate => (None, "stalemate"),
        GameOverState::InsufficientMaterial => (None, "insufficient_material"),
        GameOverState::Aborted => (None, "aborted"),
        GameOverState::Playing => return,
    };

    game_ended_events.write(GameEndedEvent {
        game_id,
        winner,
        reason: reason.to_string(),
    });
}

/// Records a bot game's result to the backend, purely for history — no
/// on-chain effect, no Elo change (on-chain Elo stays driven only by real
/// wagered/ranked settlement). Deliberately NOT gated behind the `solana`
/// feature: this is an Account-only concept, independent of wallets. Skips
/// entirely for Guest play or when not logged in — see
/// docs/plans/identity-implementation-plan.md.
///
/// Local pass-and-play (`GameMode::MultiplayerLocal`) is intentionally not
/// recorded here: both sides are the same physical player, so "did the
/// Account win or lose" isn't well-defined the way it is for a bot game.
pub fn record_casual_game_on_end(
    game_over: Res<GameOverState>,
    game_mode: Res<crate::core::states::GameMode>,
    players: Option<Res<crate::game::resources::player::Players>>,
    player_identity: Option<Res<crate::states::main_menu::PlayerIdentity>>,
    mut recorded: Local<bool>,
) {
    if !game_over.is_game_over() {
        *recorded = false;
        return;
    }
    if *recorded {
        return;
    }
    if *game_mode != crate::core::states::GameMode::SinglePlayer {
        return;
    }
    *recorded = true;

    let Some(identity) = player_identity else {
        return;
    };
    if identity.is_guest {
        return;
    }
    let Some(token) = identity.jwt_token.clone() else {
        return;
    };

    let Some(players) = players else { return };
    let human_color = if players.player_1.is_human {
        players.player_1.color
    } else if players.player_2.is_human {
        players.player_2.color
    } else {
        return; // no human side (shouldn't happen in SinglePlayer)
    };

    let result = match game_over.winner() {
        Some(winner) if winner == human_color => "win",
        Some(_) => "loss",
        None => "draw",
    };

    let base_url = crate::multiplayer::network::vps::vps_base();
    let result = result.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "opponent_type": "bot",
            "result": result,
        });
        if let Err(e) = client
            .post(format!("{base_url}/api/games/casual"))
            .bearer_auth(token)
            .json(&body)
            .send()
        {
            warn!("[casual-game] Failed to record bot game result: {}", e);
        }
    });
}

/// Convert `NetworkEvent::MessageReceived(NetworkMessage::Move)` into `NetworkMoveEvent`
/// so that `handle_network_moves` can apply opponent moves to the local board.
/// Runs for move messages from both direct gossip and relay-backed transport.
pub fn dispatch_remote_moves(
    mut network_events: MessageReader<NetworkEvent>,
    mut move_events: MessageWriter<crate::game::events::NetworkMoveEvent>,
    game_mode: Res<crate::core::states::GameMode>,
) {
    if *game_mode != crate::core::states::GameMode::OnlineMultiplayer {
        return;
    }
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::Move {
            move_uci, next_fen, ..
        }) = event
        {
            if move_uci.len() >= 4 {
                let bytes = move_uci.as_bytes();
                let from_file = bytes[0].wrapping_sub(b'a');
                let from_rank = bytes[1].wrapping_sub(b'1');
                let to_file = bytes[2].wrapping_sub(b'a');
                let to_rank = bytes[3].wrapping_sub(b'1');
                let promotion = move_uci.get(4..5).and_then(|s| s.chars().next());
                info!(
                    "[NET] Dispatching remote move {} as NetworkMoveEvent",
                    move_uci
                );
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
            committed_fen, ..
        }) = event
        {
            warn!(
                "[NET] Applying ResyncResponse — overwriting local engine with FEN: {}",
                committed_fen
            );
            let _ = engine.set_from_fen(committed_fen);
            *selection = crate::game::resources::Selection::default();
        }
    }
}

/// Route inbound draw, timeout, rematch, and ping/pong messages into Bevy events.
pub fn handle_game_control_messages(
    mut network_events: MessageReader<NetworkEvent>,
    mut draw_offer: MessageWriter<crate::game::events::DrawOfferEvent>,
    mut draw_response: MessageWriter<crate::game::events::DrawResponseEvent>,
    mut rematch_offer: MessageWriter<crate::game::events::RematchOfferEvent>,
    mut rematch_response: MessageWriter<crate::game::events::RematchResponseEvent>,
    mut flag_timeout: MessageWriter<crate::game::events::FlagTimeoutEvent>,
    network_state: Res<OnlineNetworkState>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    for event in network_events.read() {
        let NetworkEvent::MessageReceived(msg) = event else {
            continue;
        };

        match msg {
            NetworkMessage::DrawOffer { player, .. } => {
                draw_offer.write(crate::game::events::DrawOfferEvent {
                    player: player.clone(),
                    remote: true,
                });
            }
            NetworkMessage::DrawResponse {
                player, accepted, ..
            } => {
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
            NetworkMessage::RematchResponse {
                player, accepted, ..
            } => {
                rematch_response.write(crate::game::events::RematchResponseEvent {
                    player: player.clone(),
                    accepted: *accepted,
                    remote: true,
                });
            }
            NetworkMessage::Ping {
                game_id,
                timestamp_ms,
            } => {
                // Reply with Pong immediately. Gossip-only: a live Braid
                // subscription is itself a connectivity-liveness signal now
                // (see `braid_transport`'s module doc comment), so Ping/Pong
                // doesn't need a durable fallback the way moves/resign/
                // session-handshake do.
                let pong = NetworkMessage::Pong {
                    game_id: *game_id,
                    timestamp_ms: *timestamp_ms,
                };
                if let Some(tx) = &network_state.message_sender {
                    let _ = tx.send(pong);
                }
            }
            _ => {}
        }
    }
}

/// Forward local draw offers, draw responses, rematch messages, flag
/// timeouts, and resignations to the network.
pub fn send_local_draw_events(
    mut local_draw_offers: MessageReader<crate::game::events::DrawOfferEvent>,
    mut local_draw_responses: MessageReader<crate::game::events::DrawResponseEvent>,
    mut local_rematch_offers: MessageReader<crate::game::events::RematchOfferEvent>,
    mut local_rematch_responses: MessageReader<crate::game::events::RematchResponseEvent>,
    mut local_flag_timeouts: MessageReader<crate::game::events::FlagTimeoutEvent>,
    mut local_resigns: MessageReader<crate::game::events::ResignEvent>,
    network_state: Res<OnlineNetworkState>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
) {
    use crate::multiplayer::network::protocol::NetworkMessage;

    let game_id = session.as_ref().map_or(0, |s| {
        crate::multiplayer::network::online_game_session::numeric_game_id(&s.game_id)
    });
    let Some(tx) = &network_state.message_sender else {
        return;
    };

    for ev in local_draw_offers.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::DrawOffer {
            game_id,
            player: ev.player.clone(),
        });
    }
    for ev in local_draw_responses.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::DrawResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_rematch_offers.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::RematchOffer {
            game_id,
            player: ev.player.clone(),
        });
    }
    for ev in local_rematch_responses.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::RematchResponse {
            game_id,
            player: ev.player.clone(),
            accepted: ev.accepted,
        });
    }
    for ev in local_flag_timeouts.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::FlagTimeout {
            game_id,
            flagged_player: ev.flagged_player.clone(),
        });
    }
    // The in-game "Resign" button (game_ui.rs) only ever wrote the local
    // ResignEvent that updates this client's own GameOverState — nothing
    // forwarded it to the opponent, so resigning ended the game on one
    // screen while the other side's clock and board just kept running.
    // `confirm_exit_game` (input.rs) already sent NetworkMessage::Resign
    // for the "exit mid-game" path; this closes the same gap for the actual
    // Resign button.
    for ev in local_resigns.read() {
        if ev.remote {
            continue;
        }
        let _ = tx.send(NetworkMessage::Resign {
            game_id,
            winner: ev.winner.clone(),
            nonce: 0, // resign doesn't need strict nonce ordering — same as input.rs's exit-resign path
        });
    }
}

/// When the opponent sends [`NetworkMessage::ResyncRequest`], reply with our current engine FEN
/// as a [`NetworkMessage::ResyncResponse`] so they can snap back to the correct board state.
pub fn handle_resync_request(
    mut network_events: MessageReader<NetworkEvent>,
    engine: Res<crate::engine::board_state::ChessEngine>,
    network_state: Res<OnlineNetworkState>,
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
    network_state: Res<OnlineNetworkState>,
    session: Option<Res<crate::multiplayer::network::online_game_session::OnlineGameSession>>,
    game_mode: Res<crate::core::states::GameMode>,
    p2p_conn: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    mut game_over: ResMut<crate::game::resources::history::game_over::GameOverState>,
    mut next_state: ResMut<NextState<crate::core::GameState>>,
    braid_state: Res<crate::multiplayer::network::braid_transport::BraidTransportState>,
) {
    use crate::core::states::GameMode;
    use crate::multiplayer::network::protocol::NetworkMessage;

    if *game_mode != GameMode::OnlineMultiplayer {
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
            .subsec_millis()
            % 1000) as f32
            / 1000.0
            - 0.5;
        heartbeat.ping_interval = 5.0 + jitter;
        heartbeat.since_last_ping = 0.0;
        if let Some(sess) = session.as_ref() {
            let game_id =
                crate::multiplayer::network::online_game_session::numeric_game_id(&sess.game_id);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let ping = NetworkMessage::Ping {
                game_id,
                timestamp_ms: ts,
            };
            // Gossip-only now — see the timeout check below, which no
            // longer fires on gossip silence alone when the Braid
            // subscription is confirmed alive.
            if let Some(tx) = &network_state.message_sender {
                let _ = tx.send(ping);
            }
        }
    }

    // Declare disconnect if pong is overdue over gossip AND the Braid
    // moves/chat subscription is also down — otherwise this is exactly the
    // "relay-only" case the old relay's Ping/Pong fallback existed to
    // prevent from false-positiving: gossip's Ping/Pong genuinely isn't
    // getting through, but the opponent's moves are still arriving via
    // Braid, so the game is not actually abandoned.
    if heartbeat.since_last_pong >= heartbeat.timeout_secs && !braid_state.is_connected() {
        heartbeat.timed_out = true;
        warn!(
            "[NET] Heartbeat timeout — opponent disconnected after {:.0}s silence",
            heartbeat.since_last_pong
        );
        // Treat as a win for the local player (opponent abandoned). Which color
        // that is depends on which side we're playing, tracked on connect in
        // `p2p::handle_network_events` (host=White, joiner=Black).
        use crate::game::resources::history::game_over::GameOverState;
        use crate::rendering::pieces::PieceColor;
        *game_over = match p2p_conn.as_ref().and_then(|c| c.player_color) {
            Some(PieceColor::White) => GameOverState::WhiteWonByAbandonment,
            Some(PieceColor::Black) => GameOverState::BlackWonByAbandonment,
            None => GameOverState::Aborted,
        };
        next_state.set(crate::core::GameState::GameOver);
    }
}

/// Reset heartbeat counters when a Pong arrives.
pub fn handle_pong(
    mut network_events: MessageReader<NetworkEvent>,
    mut heartbeat: ResMut<HeartbeatState>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(
            crate::multiplayer::network::protocol::NetworkMessage::Pong { .. },
        ) = event
        {
            heartbeat.since_last_pong = 0.0;
        }
    }
}

/// Clear per-match networking state when leaving `InGame`, so the *next*
/// match doesn't inherit it. Before this, three resources persisted across
/// games in the same process:
///
/// - `P2PConnectionState.status` stayed `Connected`/`InGame` from the last
///   match, so `handle_connect_to_peer`'s dedup guard silently dropped the
///   new match's `ConnectToPeerEvent` ("Ignoring duplicate connect request —
///   already InGame"), leaving direct Iroh P2P never established.
/// - `RelayBridge.poll_index` kept counting up across games, so polling the
///   new (empty) relay mailbox with a stale high `since` index meant nothing
///   ever came back — pongs and moves were silently swallowed.
/// - `HeartbeatState` (`timed_out` latch / stale `since_last_pong`) carried
///   over, so a match that inherits both broken transports above has no
///   working path to a Pong and hits the 15s abandonment timeout almost
///   immediately.
///
/// Together these made every match after the first in a session (e.g. a
/// wagered game played after a casual one) silently lose connectivity and
/// get declared "opponent disconnected" within seconds of starting.
pub fn reset_multiplayer_session_state(
    mut p2p_conn: ResMut<crate::multiplayer::network::p2p::P2PConnectionState>,
    mut heartbeat: ResMut<HeartbeatState>,
    mut braid_transport: ResMut<crate::multiplayer::network::braid_transport::BraidTransportState>,
) {
    *p2p_conn = crate::multiplayer::network::p2p::P2PConnectionState::default();
    *heartbeat = HeartbeatState::default();
    braid_transport.reset();
    info!("[NET] Reset P2P connection, heartbeat, and Braid transport state on match exit");
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
        let sk = SecretKey::generate();
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
mod gossip_mesh_tests {
    use super::*;

    fn peer(byte: u8) -> EndpointId {
        SecretKey::from_bytes(&[byte; 32]).public()
    }

    /// The bug this exists to prevent: moves are broadcast on
    /// `GAME_TOPIC/<game_id>`, but the only `join_peers` call was against the
    /// global `GAME_TOPIC`. An iroh-gossip swarm is per-`TopicId`, so the
    /// per-game topic had one member on each side and delivered nothing.
    #[test]
    fn a_peer_learned_after_subscribing_is_joined_into_the_game_topic() {
        let mut mesh = GossipMesh::default();
        mesh.topics.insert(GAME_TOPIC.to_string());

        // Game topic joined before the opponent is known — the common order,
        // since `start_session` runs at lobby time.
        let game_topic = format!("{}/{}", GAME_TOPIC, 12345u64);
        let bootstrap = mesh.subscribe(&game_topic);
        assert!(bootstrap.is_empty(), "no peers known yet");

        // Opponent arrives via the bootstrap channel.
        let topics = mesh.add_peer(peer(7));
        assert!(
            topics.contains(&game_topic),
            "the per-game move topic must be back-filled, not just {GAME_TOPIC}"
        );
        assert!(topics.contains(&GAME_TOPIC.to_string()));
    }

    /// The other ordering: peer first, then the topic. The subscribe must
    /// carry the known peer as bootstrap, since nothing will re-join it later.
    #[test]
    fn a_topic_subscribed_after_the_peer_is_known_bootstraps_with_it() {
        let mut mesh = GossipMesh::default();
        mesh.add_peer(peer(3));

        let game_topic = format!("{}/{}", GAME_TOPIC, 999u64);
        let bootstrap = mesh.subscribe(&game_topic);

        assert_eq!(
            bootstrap,
            vec![peer(3)],
            "subscribing with an empty bootstrap list joins a swarm of one"
        );
    }

    /// Every peer must reach every topic, regardless of interleaving.
    #[test]
    fn all_known_peers_reach_all_known_topics() {
        let mut mesh = GossipMesh::default();
        mesh.add_peer(peer(1));
        let t1 = format!("{}/{}", GAME_TOPIC, 1u64);
        assert_eq!(mesh.subscribe(&t1), vec![peer(1)]);

        let topics = mesh.add_peer(peer(2));
        assert!(topics.contains(&t1), "peer 2 must be joined into t1");

        let t2 = format!("{}/{}", GAME_TOPIC, 2u64);
        let bootstrap = mesh.subscribe(&t2);
        assert_eq!(bootstrap.len(), 2, "t2 must bootstrap with both peers");
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    /// A1 regression: an attacker who signs with their OWN key but stuffs a
    /// victim's identity into `signer_pubkey` must not be able to act as the victim.
    /// `bind_identity` discards the claimed `signer_pubkey` and substitutes the
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
            signer_pubkey: victim_id.clone(), // forged: claims to be the victim
            seq: 1,
            parent_version: "0".to_string(),
        };

        let signed = SignedNetworkMessage::sign(msg, &attacker_sk);
        // The signature IS valid — for the attacker's own key.
        assert!(signed.verify());
        let attacker_pub = signed.session_pubkey.clone();

        let bound = bind_identity(signed);
        match bound {
            NetworkMessage::Move { signer_pubkey, .. } => {
                assert_eq!(
                    signer_pubkey, attacker_pub,
                    "signer_pubkey must be the verified signer"
                );
                assert_ne!(
                    signer_pubkey, victim_id,
                    "the forged victim identity must be discarded"
                );
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
        assert!(matches!(
            bind_identity(signed),
            NetworkMessage::ResyncRequest { game_id: 42 }
        ));
    }
}

/// docs/PRE_MAINNET_E2E_PLAN.md §1.2/§6.3: property test for cross-transport
/// move dedup — the plan's own assessment calls this "the highest-value
/// single test in this whole plan." Drives the REAL `handle_network_events`
/// (gossip), `drain_braid_messages` (Braid relay), and `dispatch_remote_moves`
/// (the shared consumer both feed) systems inside a headless Bevy `App`
/// against a shared `CausalChainState`, so this exercises the actual
/// production dedup mechanism (`applied_versions`), not a reimplementation.
///
/// Scope: a single canonical move stream (one fixed agent identity, so the
/// per-agent seq/parent-version causal chain — gossip's own replay/fork
/// guard — stays internally consistent) is delivered through an arbitrary
/// mix of gossip-only, relay-only, both, and duplicated-within-a-transport
/// patterns, each transport's arrival order independently shuffled. This
/// exercises: (a) `NonceSequencer`'s gossip reorder gate on out-of-order
/// gossip arrivals, (b) relay's order-independence, and (c) cross-transport
/// dedup via `applied_versions` for moves delivered by both. Not covered
/// (noted as follow-up, not silently assumed safe): a deliberately
/// *malicious* forged relay message reusing a legitimate move's
/// `version_hash` — see the module doc's "smuggle a different version_hash"
/// concern, which needs an adversarial (not just reordering/duplication)
/// scenario.
#[cfg(test)]
mod dual_transport_dedup_property_tests {
    use super::*;
    use crate::core::states::GameMode;
    use crate::game::events::NetworkMoveEvent;
    use crate::multiplayer::network::braid_transport::{drain_braid_messages, BraidTransportState};
    use crate::multiplayer::network::online_game_session::OnlineGameSession;
    use braid_chess::message::{ChessMessage, MovePayload};
    use proptest::prelude::*;

    const GAME_ID: u64 = 777;
    const AGENT_ID: &[u8] = &[1, 2, 3, 4];

    fn fen_for(i: usize) -> String {
        format!("fen_{i}")
    }
    fn turn_for(i: usize) -> u16 {
        (i + 1) as u16
    }
    /// The version_hash a move at index `i` produces — identical formula to
    /// both `handle_network_events` (gossip) and `drain_braid_messages`
    /// (relay), which is exactly what makes cross-transport dedup possible.
    fn head_for(i: usize) -> String {
        braid_chess::version_hash(&fen_for(i), turn_for(i) as u32)
    }
    fn parent_for(i: usize) -> String {
        if i == 0 {
            "0".to_string()
        } else {
            head_for(i - 1)
        }
    }

    fn gossip_move(i: usize) -> NetworkMessage {
        NetworkMessage::Move {
            game_id: GAME_ID,
            turn: turn_for(i),
            move_uci: "e2e4".to_string(),
            next_fen: fen_for(i),
            nonce: (i + 1) as u64,
            timestamp_ms: 0,
            signer_pubkey: AGENT_ID.to_vec(),
            seq: (i + 1) as u64,
            parent_version: parent_for(i),
        }
    }

    fn relay_move(i: usize) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci(
            "e2e4",
            fen_for(i),
            turn_for(i) as u32,
            "alice",
        ))
    }

    /// How a single canonical move gets delivered across the two transports.
    #[derive(Debug, Clone, Copy)]
    enum Delivery {
        GossipOnly,
        RelayOnly,
        Both,
        GossipTwice,
        RelayTwice,
        BothRelayTwice,
    }

    fn delivery_strategy() -> impl Strategy<Value = Delivery> {
        prop_oneof![
            Just(Delivery::GossipOnly),
            Just(Delivery::RelayOnly),
            Just(Delivery::Both),
            Just(Delivery::GossipTwice),
            Just(Delivery::RelayTwice),
            Just(Delivery::BothRelayTwice),
        ]
    }

    /// Deterministic Fisher-Yates shuffle seeded from a proptest-generated
    /// `u64`, so failures reproduce exactly (proptest doesn't ship an
    /// off-the-shelf `Vec` shuffle combinator, and pulling in `rand` just for
    /// this one shuffle would defeat the point of a minimal, scoped
    /// dependency addition — see this module's parent doc comment).
    fn shuffle<T>(items: &mut Vec<T>, seed: u64) {
        let mut state = seed | 1; // xorshift64 needs a nonzero seed
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for i in (1..items.len()).rev() {
            let j = (next() as usize) % (i + 1);
            items.swap(i, j);
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameMode::OnlineMultiplayer);
        app.insert_resource(CausalChainState::default());
        app.insert_resource(PendingMoveBuffer::default());
        app.insert_resource(OnlineGameSession {
            game_id: GAME_ID.to_string(),
            ..Default::default()
        });
        app.add_message::<NetworkEvent>();
        app.add_message::<ResignEvent>();
        app.add_message::<crate::multiplayer::network::online_game_session::OnlineChatMessage>();
        app.add_message::<NetworkMoveEvent>();
        app.add_systems(
            Update,
            (
                handle_network_events,
                drain_braid_messages,
                dispatch_remote_moves,
            )
                .chain(),
        );
        app
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]
        #[test]
        fn dual_transport_delivers_each_move_exactly_once(
            move_count in 1usize..8,
            deliveries in prop::collection::vec(delivery_strategy(), 1..8),
            gossip_seed in any::<u64>(),
            relay_seed in any::<u64>(),
        ) {
            let move_count = move_count.min(deliveries.len());
            prop_assume!(move_count > 0);
            let mut deliveries = deliveries;

            // Once gossip misses one move for this agent (a pure-relay
            // delivery), gossip's own per-agent seq chain has a permanent gap
            // for this receiver — the *next* gossip message would be
            // correctly rejected as a seq gap (a real, already-covered
            // mechanism: `NonceSequencer`'s `Overflow{resync_from}` triggers
            // a resync). That's a gap-recovery/liveness concern, not the
            // cross-transport *dedup* property this test targets — so once a
            // gossip miss occurs, every later move is normalized to also
            // skip gossip, keeping the chain (and this test's scope)
            // consistent instead of tripping an unrelated, already-tested
            // rejection path.
            let mut gossip_has_gapped = false;
            for d in deliveries.iter_mut().take(move_count) {
                if gossip_has_gapped {
                    *d = match *d {
                        Delivery::GossipOnly | Delivery::Both => Delivery::RelayOnly,
                        Delivery::GossipTwice | Delivery::BothRelayTwice => Delivery::RelayTwice,
                        other @ (Delivery::RelayOnly | Delivery::RelayTwice) => other,
                    };
                }
                if matches!(*d, Delivery::RelayOnly | Delivery::RelayTwice) {
                    gossip_has_gapped = true;
                }
            }

            let mut gossip_items: Vec<NetworkEvent> = Vec::new();
            let mut relay_items: Vec<ChessMessage> = Vec::new();

            for i in 0..move_count {
                match deliveries[i] {
                    Delivery::GossipOnly => {
                        gossip_items.push(NetworkEvent::MessageReceived(gossip_move(i)));
                    }
                    Delivery::RelayOnly => {
                        relay_items.push(relay_move(i));
                    }
                    Delivery::Both => {
                        gossip_items.push(NetworkEvent::MessageReceived(gossip_move(i)));
                        relay_items.push(relay_move(i));
                    }
                    Delivery::GossipTwice => {
                        gossip_items.push(NetworkEvent::MessageReceived(gossip_move(i)));
                        gossip_items.push(NetworkEvent::MessageReceived(gossip_move(i)));
                    }
                    Delivery::RelayTwice => {
                        relay_items.push(relay_move(i));
                        relay_items.push(relay_move(i));
                    }
                    Delivery::BothRelayTwice => {
                        gossip_items.push(NetworkEvent::MessageReceived(gossip_move(i)));
                        relay_items.push(relay_move(i));
                        relay_items.push(relay_move(i));
                    }
                }
            }

            // Gossip arrivals are shuffled — NonceSequencer must reorder them
            // back into strict nonce order before the causal chain ever sees
            // them. Relay arrivals are shuffled too, since Braid needs no
            // ordering at all by design.
            shuffle(&mut gossip_items, gossip_seed);
            shuffle(&mut relay_items, relay_seed);

            let (gossip_tx, gossip_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
            let (relay_tx, relay_rx) = crossbeam_channel::unbounded::<ChessMessage>();

            for item in gossip_items {
                gossip_tx.send(item).unwrap();
            }
            for item in relay_items {
                relay_tx.send(item).unwrap();
            }

            let mut app = build_app();
            app.insert_resource({
                let mut ns = OnlineNetworkState::default();
                ns.event_receiver = Some(gossip_rx);
                ns
            });
            app.insert_resource(BraidTransportState::new_for_test(
                GAME_ID.to_string(),
                relay_rx,
            ));

            // `.chain()` on the three systems above means `dispatch_remote_moves`
            // (last in the chain) already sees, within this same update, every
            // message the first two wrote — one `update()` is enough. A second
            // no-op update is added defensively; Bevy retains messages for 2
            // frames after they're written, so nothing from the first update
            // is dropped by the time this reads them.
            app.update();
            app.update();

            let emitted = app.world().resource::<Messages<NetworkMoveEvent>>();
            let mut reader = bevy::ecs::message::MessageCursor::<NetworkMoveEvent>::default();
            let all_fens: Vec<String> = reader
                .read(emitted)
                .filter_map(|e| e.expected_fen.clone())
                .collect();

            for i in 0..move_count {
                let expected = fen_for(i);
                let count = all_fens.iter().filter(|f| **f == expected).count();
                prop_assert_eq!(
                    count, 1,
                    "move {} (fen {:?}) must be applied exactly once, was applied {} times",
                    i, expected, count
                );
            }
            prop_assert_eq!(
                all_fens.len(), move_count,
                "no move outside the canonical set should ever be emitted"
            );
        }
    }
}

/// Phase C (`docs/plans/networking-hardening-plan.md`): once
/// `CausalChainState::verified_wallets` is populated for a game, a
/// `SessionInfo` claiming a `player_pubkey` that isn't actually white or
/// black on-chain must not get its `signing_pubkey` seated on the roster —
/// this is the client-side mirror of Phase B's backend
/// `on_chain_verified_roster_beats_a_forged_first_claim` regression test.
/// Only meaningful under `--features solana`: `verified_wallets` is only
/// ever populated by `spawn_verified_participants_fetch`/
/// `poll_verified_participants_fetch`, both solana-gated — without the
/// feature the map stays empty for every game and the roster keeps its
/// original trust-first bootstrap unconditionally (already covered by every
/// other roster test in this file).
#[cfg(all(test, feature = "solana"))]
mod verified_wallets_roster_tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    const GAME_ID: u64 = 1;

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(CausalChainState::default());
        app.insert_resource(PendingMoveBuffer::default());
        app.add_message::<NetworkEvent>();
        app.add_message::<ResignEvent>();
        app.add_systems(Update, handle_network_events);
        app
    }

    fn session_info(player_pubkey: Pubkey, signing_pubkey: Pubkey) -> NetworkMessage {
        NetworkMessage::SessionInfo {
            game_id: GAME_ID,
            player_pubkey,
            session_pubkey: Pubkey::new_unique(),
            signing_pubkey,
            expires_at: i64::MAX,
        }
    }

    #[test]
    fn forged_player_pubkey_is_not_seated_on_the_roster() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let mallory = Pubkey::new_unique(); // neither white nor black
        let mallory_signing = Pubkey::new_unique();

        let mut app = build_app();
        {
            let mut causal = app.world_mut().resource_mut::<CausalChainState>();
            causal
                .verified_wallets
                .insert(GAME_ID, (white.to_string(), black.to_string()));
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
        app.insert_resource({
            let mut ns = OnlineNetworkState::default();
            ns.event_receiver = Some(event_rx);
            ns
        });
        event_tx
            .send(NetworkEvent::MessageReceived(session_info(
                mallory,
                mallory_signing,
            )))
            .unwrap();

        app.update();

        let causal = app.world().resource::<CausalChainState>();
        let roster = causal.roster.get(&GAME_ID);
        let mallory_key = mallory_signing.to_bytes().to_vec();
        assert!(
            roster.is_none_or(|r| !r.contains(&mallory_key)),
            "a SessionInfo whose player_pubkey isn't on-chain white/black must not seat its \
             signing_pubkey on the roster, even though the PUT/claim itself is never rejected outright"
        );
    }

    #[test]
    fn genuine_player_pubkey_is_seated_on_the_roster() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let white_signing = Pubkey::new_unique();

        let mut app = build_app();
        {
            let mut causal = app.world_mut().resource_mut::<CausalChainState>();
            causal
                .verified_wallets
                .insert(GAME_ID, (white.to_string(), black.to_string()));
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
        app.insert_resource({
            let mut ns = OnlineNetworkState::default();
            ns.event_receiver = Some(event_rx);
            ns
        });
        event_tx
            .send(NetworkEvent::MessageReceived(session_info(
                white,
                white_signing,
            )))
            .unwrap();

        app.update();

        let causal = app.world().resource::<CausalChainState>();
        let roster = causal
            .roster
            .get(&GAME_ID)
            .expect("roster must be populated");
        assert!(roster.contains(&white_signing.to_bytes().to_vec()));
    }
}

/// docs/PRE_MAINNET_E2E_PLAN.md §2.3: `SessionInfo.player_pubkey` is
/// self-asserted by the claiming peer with no wallet signature over it (see
/// `NetworkMessage::SessionInfo`'s doc comment in `network/protocol.rs`).
/// This documents that gap directly, then pairs it with an on-chain test
/// (`programs/xfchess-game/tests/global_session_settlement_tests.rs`'s
/// `finalize_game_rejects_spoofed_black_authority`) proving the boundary the
/// checklist worried about doesn't actually let a spoofed identity move
/// escrow funds — the client can build a `finalize_game` call from a
/// spoofed `opponent_pubkey`, but the on-chain `constraint =
/// black_authority.key() == game.black` check rejects it regardless.
#[cfg(all(test, feature = "solana"))]
mod session_info_spoof_tests {
    use super::*;
    use crate::multiplayer::rollup::manager::EphemeralRollupManager;
    use crate::multiplayer::rollup::session_keys::HandshakeOrderingKeyManager;
    use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
    use bevy::prelude::*;
    use solana_sdk::pubkey::Pubkey;

    /// A peer can announce any `player_pubkey` alongside its own freshly
    /// generated `signing_pubkey` — `handle_session_info_from_network` never
    /// verifies that the announcing peer actually controls `player_pubkey`,
    /// it just stores whatever was claimed into `opponent_pubkey`.
    #[test]
    fn handle_session_info_accepts_a_spoofed_player_pubkey_with_no_verification() {
        let victim_wallet = Pubkey::new_unique(); // the identity being impersonated
        let attacker_signing_key = Pubkey::new_unique(); // attacker's own gossip key

        let mut app = App::new();
        app.insert_resource(EphemeralRollupManager::new(
            1,
            false,
            "startpos".to_string(),
        ));
        app.insert_resource(HandshakeOrderingKeyManager::default());
        app.insert_resource(SolanaIntegrationState::default());
        app.add_message::<NetworkEvent>();
        app.add_systems(Update, handle_session_info_from_network);

        app.world_mut()
            .write_message(NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
                game_id: 1,
                player_pubkey: victim_wallet, // claimed, unverified
                session_pubkey: Pubkey::new_unique(),
                signing_pubkey: attacker_signing_key,
                expires_at: i64::MAX,
            }));
        app.update();

        let state = app.world().resource::<SolanaIntegrationState>();
        assert_eq!(
            state.opponent_pubkey,
            Some(victim_wallet),
            "the real gap: opponent_pubkey is set from the claimed player_pubkey with no \
             signature tying it to the sender — see docs/PRE_MAINNET_E2E_PLAN.md §2.3"
        );
    }
}
