use super::*;

#[derive(Default)]
pub(super) struct GossipMesh {
    pub(super) peers: std::collections::HashSet<EndpointId>,
    pub(super) topics: std::collections::HashSet<String>,
}

impl GossipMesh {
    pub(super) fn subscribe(&mut self, topic: &str) -> Vec<EndpointId> {
        self.topics.insert(topic.to_string());
        self.peers.iter().copied().collect()
    }

    pub(super) fn add_peer(&mut self, peer: EndpointId) -> Vec<String> {
        self.peers.insert(peer);
        self.topics.iter().cloned().collect()
    }
}

pub const GAME_TOPIC: &str = "/xfchess-game";

pub fn initialize_braid_network(
    mut network_state: ResMut<OnlineNetworkState>,
    tokio_runtime: Res<TokioRuntime>,
    player_identity: Res<crate::states::main_menu::PlayerIdentity>,
) {
    if network_state.connected
        || network_state.initialization_in_progress
        || player_identity.username.is_none()
    {
        return;
    }

    network_state.initialization_in_progress = true;

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkEvent>();
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<NetworkMessage>();
    let (bootstrap_tx, mut bootstrap_rx) = tokio::sync::mpsc::unbounded_channel::<EndpointId>();
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    network_state.event_receiver = Some(event_rx);
    network_state.event_sender = Some(event_tx.clone()); // relay bridge injects incoming here
    network_state.message_sender = Some(msg_tx);
    network_state.bootstrap_sender = Some(bootstrap_tx.clone());
    network_state.subscription_sender = Some(sub_tx);

    // Read the shared signing-key cell on each send; boot precedes wallet connection.
    let session_signing_key_shared = network_state.session_signing_key_shared.clone();
    let event_tx_clone = event_tx;

    tokio_runtime.0.spawn(async move {
        info!("[NET] Starting Iroh node task...");
        let (secret_key, raw_bytes) = load_or_generate_key();
        // Derive node ID before consuming the key so we can set it as the
        // proxy's default_peer (browser spectators → local iroh node).
        let derived_node_id: EndpointId = secret_key.public();

        #[cfg(target_os = "android")]
        let braid_data_dir = crate::core::paths::internal_data_dir().map(|d| d.join("braid"));
        #[cfg(not(target_os = "android"))]
        let braid_data_dir = dirs::data_local_dir()
            .map(|d| d.join("xfchess").join("braid"))
            .or_else(|| Some(std::path::PathBuf::from("braid-data")));

        // Scope spectator bridge ports by XFCHESS_WALLET_PORT for concurrent local instances.
        let wallet_port: u16 = std::env::var("XFCHESS_WALLET_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7454);
        let proxy_port = 8181u16.wrapping_add(wallet_port.wrapping_sub(7454));
        let listen_addr = std::net::SocketAddr::from(([127, 0, 0, 1], proxy_port));

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
                    .send(NetworkEvent::NetworkInitializationFailed(e.to_string()))
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
                event_tx_clone
                    .send(NetworkEvent::NetworkInitializationFailed(e.to_string()))
                    .ok();
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
        let bootstrap_tx_sub = bootstrap_tx.clone();
        let bootstrap_tx_main = bootstrap_tx.clone();

        let event_tx_error = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let topic = match &msg {
                    NetworkMessage::GameInvite { .. }
                    | NetworkMessage::InviteResponse { .. }
                    | NetworkMessage::GameStart { .. } => GAME_TOPIC.to_string(),
                    _ => format!("{}/{}", GAME_TOPIC, msg.game_id()),
                };
                // Capture variant and game ID before consuming msg; avoid logging full message contents.
                let msg_kind = msg.kind_str();
                let msg_game_id = msg.game_id();

                // Read fresh every send — see `session_signing_key_shared`'s
                // doc comment for why a one-time snapshot doesn't work here.
                let session_signing_key = session_signing_key_shared
                    .read()
                    .map(|guard| *guard)
                    .unwrap_or(None);
                let signed = session_signing_key.is_some();

                // Wire prefix: 0x02 = bincode SignedNetworkMessage; 0x01 = legacy JSON NetworkMessage.
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

        let event_tx_sub = event_tx_clone.clone();
        tokio::spawn(async move {
            while let Some(topic) = sub_rx.recv().await {
                // Bootstrap known peers while holding the subscription lock so no peer is
                // lost between the snapshot and topic registration.
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
                        tokio::spawn(process_gossip_stream(
                            rx_new,
                            event_tx_inner,
                            bootstrap_tx_sub.clone(),
                        ));
                    }
                    Err(e) => {
                        error!("Failed to subscribe to topic {}: {}", topic, e);
                        mesh.topics.remove(&topic);
                    }
                }
                drop(mesh);
            }
        });

        tokio::spawn(async move {
            while let Some(peer_id) = bootstrap_rx.recv().await {
                // Join every tracked topic, including game topics subscribed before this peer was discovered.
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

        process_gossip_stream(rx, event_tx_clone, bootstrap_tx_main).await;
    });
}

pub(super) fn bind_identity(mut signed: SignedNetworkMessage) -> NetworkMessage {
    if let NetworkMessage::Move { signer_pubkey, .. } = &mut signed.msg {
        *signer_pubkey = signed.session_pubkey.clone();
    }
    signed.msg
}

pub(super) async fn process_gossip_stream(
    mut rx: iroh_gossip::api::GossipReceiver,
    event_tx: tokio::sync::mpsc::UnboundedSender<NetworkEvent>,
    bootstrap_tx: tokio::sync::mpsc::UnboundedSender<EndpointId>,
) {
    while let Some(result) = rx.next().await {
        match result {
            Ok(IrohEvent::NeighborUp(peer_id)) => {
                info!("[NET] Peer connected via gossip: {}", peer_id);
                // Bootstrap this endpoint into every tracked topic, including topics subscribed after discovery.
                bootstrap_tx.send(peer_id).ok();
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
                                // Reject unsigned messages unless allow-unsigned-p2p is enabled for local testing.
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

    #[test]
    fn fen_ply_rejects_invalid_turns_and_orders_positions() {
        assert_eq!(fen_ply("8/8/8/8/8/8/8/8 w - - 0 1"), Some(0));
        assert_eq!(fen_ply("8/8/8/8/8/8/8/8 b - - 0 1"), Some(1));
        assert_eq!(fen_ply("8/8/8/8/8/8/8/8 w - - 0 2"), Some(2));
        assert_eq!(fen_ply("8/8/8/8/8/8/8/8 x - - 0 2"), None);
        assert_eq!(fen_ply("8/8/8/8/8/8/8/8 w - - 0 0"), None);
    }

    fn peer(byte: u8) -> EndpointId {
        SecretKey::from_bytes(&[byte; 32]).public()
    }

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
