use super::*;

pub fn handle_network_events(
    mut network_state: ResMut<OnlineNetworkState>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
    mut start_barrier: ResMut<crate::multiplayer::types::OnlineStartBarrier>,
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
        // Sequence moves across gossip and relay transports before adding them to ready.
        // Do not feed released messages through the gate again. Resign bypasses the gate
        // because it is terminal, idempotent, and outside the ordered move stream.
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
                    network_state.initialization_in_progress = false;
                    info!("Braid network initialized with node ID: {}", node_id);
                }
                NetworkEvent::NetworkInitializationFailed(reason) => {
                    network_state.initialization_in_progress = false;
                    warn!("[NET] Braid network initialization failed: {}", reason);
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
                    // NonceSequencer has already ordered and replay-checked moves before this point.
                    let game_id = msg.game_id();

                    match msg {
                        NetworkMessage::GameReady {
                            game_id: ready_game,
                            player_pubkey,
                            ready_token,
                        } if *ready_game == game_id
                            && *ready_token == format!("game-{game_id}") =>
                        {
                            let wallet_allowed = match causal.verified_wallets.get(ready_game) {
                                Some((white, black)) => {
                                    let claimed = player_pubkey.to_string();
                                    &claimed == white || &claimed == black
                                }
                                None => true,
                            };
                            if wallet_allowed {
                                start_barrier.remote_ready = true;
                                info!("[START] Remote player ready for game {game_id}");
                                if start_barrier.local_ready && !start_barrier.start_sent {
                                    if let Some(tx) = &network_state.message_sender {
                                        if tx
                                            .send(NetworkMessage::GameStartConfirmed {
                                                game_id,
                                                start_token: format!("game-{game_id}"),
                                            })
                                            .is_ok()
                                        {
                                            start_barrier.start_sent = true;
                                            start_barrier.start_confirmed = true;
                                            info!("[START] Sent synchronized start for game {game_id}");
                                        }
                                    }
                                }
                            } else {
                                warn!(
                                    "[START] Rejected readiness for game {game_id}: wallet is not a participant"
                                );
                            }
                        }
                        NetworkMessage::GameStartConfirmed {
                            game_id: start_game,
                            start_token,
                        } if *start_game == game_id
                            && *start_token == format!("game-{game_id}")
                            && start_barrier.local_ready
                            && start_barrier.remote_ready =>
                        {
                            start_barrier.start_confirmed = true;
                            info!("[START] Synchronized start confirmed for game {game_id}");
                        }
                        _ => {}
                    }

                    // Build a two-participant roster from gossip signing_pubkey, which must
                    // match the verified envelope signer, not the session delegation key.
                    if let NetworkMessage::SessionInfo {
                        game_id: sg,
                        player_pubkey,
                        signing_pubkey,
                        ..
                    } = msg
                    {
                        // When on-chain participants are known, reject claims for other wallets.
                        // Casual games or pending lookups retain the bootstrap path.
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
                            // After roster establishment, accept moves only from its verified signers.
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
                            let (last, our_head) = causal.lane(&agent_key);
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
                            // Every noninitial move must name the current head, even if it claims a
                            // genesis parent. See specs/causal/CausalChain.tla.
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
                            let already_applied = causal
                                .applied_versions
                                .get(&game_id)
                                .is_some_and(|versions| versions.contains(&new_head));
                            let first_time_seen = !already_applied
                                && causal
                                    .pending_versions
                                    .entry(game_id)
                                    .or_default()
                                    .insert(new_head.clone());
                            causal.head_version.insert(agent_key, new_head);
                            // Skip moves already applied through Braid, identified by their resulting position.
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
                            ..
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
                        NetworkMessage::InviteResponse {
                            game_id, accepted, ..
                        } => {
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
                            ..
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
            move_uci,
            next_fen,
            turn,
            ..
        })
        | NetworkEvent::BraidMove(NetworkMessage::Move {
            move_uci,
            next_fen,
            turn,
            ..
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
                    dedup_version: Some(braid_chess::version_hash(next_fen, *turn as u32)),
                });
            } else {
                warn!("[NET] Received malformed UCI move: {:?}", move_uci);
            }
        }
    }
}

pub(crate) fn fen_ply(fen: &str) -> Option<u64> {
    let mut fields = fen.split_whitespace();
    fields.next()?;
    let side = fields.next()?;
    fields.next()?;
    fields.next()?;
    fields.next()?;
    let fullmove: u64 = fields.next()?.parse().ok()?;
    let base = fullmove.checked_sub(1)?.checked_mul(2)?;
    match side {
        "w" => Some(base),
        "b" => base.checked_add(1),
        _ => None,
    }
}

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
        app.init_resource::<crate::multiplayer::types::OnlineStartBarrier>();
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

            // After a gossip-only sequence gap, use relay copies for the remainder of
            // this dedup test. Gap recovery is covered separately.
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

            // Shuffle both transports; NonceSequencer must restore strict gossip nonce order
            // before the causal chain sees messages.
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

            // The system chain delivers messages in one update. A second update remains
            // within Bevy's two-frame message retention window.
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

#[cfg(all(test, feature = "solana"))]
mod verified_wallets_roster_tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    const GAME_ID: u64 = 1;

    fn build_app() -> App {
        let mut app = App::new();
        app.insert_resource(CausalChainState::default());
        app.insert_resource(PendingMoveBuffer::default());
        app.init_resource::<crate::multiplayer::types::OnlineStartBarrier>();
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
