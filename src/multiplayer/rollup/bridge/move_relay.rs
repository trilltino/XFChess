use super::*;

#[derive(Resource, Default, Clone)]
pub struct RecentTransactions {
    pub entries: Vec<(String, String)>,
}

impl RecentTransactions {
    const MAX: usize = 8;

    pub fn push(&mut self, move_uci: String, sig: String) {
        if self.entries.len() >= Self::MAX {
            self.entries.remove(0);
        }
        self.entries.push((move_uci, sig));
    }
}

pub(super) fn send_network_msg(state: &OnlineNetworkState, msg: NetworkMessage) {
    if let Some(tx) = &state.message_sender {
        if let Err(e) = tx.send(msg) {
            warn!("Failed to send NetworkMessage: {}", e);
        }
    }
}

pub(super) fn resolve_white_black(
    is_creator: bool,
    solana_state: Option<&SolanaIntegrationState>,
) -> Option<(Pubkey, Pubkey)> {
    let s = solana_state?;
    let local = s.wallet_pubkey?;
    let opponent = s.opponent_pubkey?;
    Some(if is_creator {
        (local, opponent)
    } else {
        (opponent, local)
    })
}

/// The mover's authorisation of `uci` as ply `ply`, if the log holds it.
fn logged_auth(log: &[braid_chess::MovePayload], ply: u64, uci: &str) -> Option<String> {
    log.iter()
        .find(|m| {
            m.uci.eq_ignore_ascii_case(uci)
                && crate::multiplayer::systems::fen_ply(&m.fen_after) == Some(ply)
        })
        .and_then(|m| m.auth.clone())
}

/// Records moves through the backend for the host. A move by the other player
/// goes with that player's own signature, which their client published in the
/// durable game log; the backend refuses to record it without.
pub(super) struct MoveRecorder {
    game_id: u64,
    me: Option<Pubkey>,
    log: Option<Vec<braid_chess::MovePayload>>,
}

impl MoveRecorder {
    pub(super) fn new(game_id: u64, me: Option<Pubkey>) -> Self {
        Self {
            game_id,
            me,
            log: None,
        }
    }

    fn auth_for(&mut self, ply: u64, uci: &str) -> Option<String> {
        if let Some(auth) = self.log.as_deref().and_then(|l| logged_auth(l, ply, uci)) {
            return Some(auth);
        }
        // Not in the copy we hold (or none yet): the opponent's publish may
        // have landed since, so read the log again.
        self.log = crate::multiplayer::vps_client::fetch_move_log(self.game_id).ok();
        self.log.as_deref().and_then(|l| logged_auth(l, ply, uci))
    }

    pub(super) fn record(
        &mut self,
        ply: u64,
        uci: &str,
        next_fen: &str,
        mover: Pubkey,
    ) -> Result<(String, String), String> {
        let auth = if Some(mover) == self.me {
            None
        } else {
            self.auth_for(ply, uci)
        };
        crate::multiplayer::vps_client::record_move(
            self.game_id,
            uci,
            next_fen,
            ply,
            &mover.to_string(),
            auth.as_deref(),
        )
    }

    /// `record`, retried twice. For the game-ending batch, which nothing
    /// flushes again if a move fails (e.g. the opponent's last signature is
    /// still in flight to the log).
    fn record_with_retry(
        &mut self,
        ply: u64,
        uci: &str,
        next_fen: &str,
        mover: Pubkey,
    ) -> Result<(String, String), String> {
        let mut delay = std::time::Duration::from_secs(2);
        let mut attempt = 1;
        loop {
            match self.record(ply, uci, next_fen, mover) {
                Err(e) if attempt < 3 => {
                    warn!(
                        "[VPS] record_move {} game {} attempt {attempt} failed: {e}",
                        uci, self.game_id
                    );
                    std::thread::sleep(delay);
                    delay *= 2;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }
}

pub(super) fn mover_wallet_for_ply(nonce: u64, white: Pubkey, black: Pubkey) -> Pubkey {
    if nonce % 2 == 1 {
        white
    } else {
        black
    }
}

pub(super) fn handle_rollup_to_network_events(
    mut rollup_events: MessageReader<RollupEvent>,
    network_state: Res<OnlineNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    rollup_manager: Res<EphemeralRollupManager>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
) {
    let er_endpoint = magicblock_resolver.er_endpoint().to_string();
    let white_black = resolve_white_black(rollup_manager.is_creator, solana_state.as_deref());
    let me = solana_state.as_deref().and_then(|s| s.wallet_pubkey);
    for event in rollup_events.read() {
        match event {
            RollupEvent::BatchReady {
                game_id,
                moves,
                next_fens,
            } => {
                let batch_hash = calculate_batch_hash(
                    *game_id,
                    rollup_manager.committed_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchPropose {
                        game_id: *game_id,
                        start_turn: rollup_manager.committed_turn,
                        moves: moves.clone(),
                        next_fens: next_fens.clone(),
                    },
                );
                bridge
                    .pending_batches
                    .insert(batch_hash.clone(), (moves.clone(), next_fens.clone()));
                bridge.sent_batch_hashes.insert(batch_hash.clone());
                bridge.last_sent_batch_hash = Some(batch_hash);
                bridge.awaiting_commit_confirmation = true;
                info!("Sent BatchPropose for game {}", game_id);
            }
            // Submit the final batch directly so peer disconnection cannot prevent recording game-ending moves.
            RollupEvent::GameEndBatch {
                game_id,
                moves,
                next_fens,
            } => {
                let Some((white_pk, black_pk)) = white_black else {
                    warn!(
                        "[VPS] Game-end batch for game {} dropped — no wallet state to attribute movers",
                        game_id
                    );
                    continue;
                };
                let gid = *game_id;
                let base_nonce = bridge.move_nonce;
                bridge.move_nonce += moves.len() as u64;
                let moves_owned = moves.clone();
                let fens_owned = next_fens.clone();
                let er_endpoint = er_endpoint.clone();
                info!(
                    "[VPS] Game-end direct submit: {} moves for game {}",
                    moves_owned.len(),
                    gid
                );
                // Block undelegation until the final move batch has been attempted.
                // poll_game_end_flush clears this flag when flush_tx fires.
                bridge.game_end_moves_flushing = true;
                let (flush_tx, flush_rx) = oneshot::channel::<()>();
                bridge.game_end_flush_rx = Some(flush_rx);
                crate::core::net_task::spawn_detached("bridge", move || {
                    use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
                    let mut recorder = MoveRecorder::new(gid, me);
                    for (i, (mv, fen)) in moves_owned.iter().zip(fens_owned.iter()).enumerate() {
                        let ply = base_nonce + i as u64;
                        let mover = mover_wallet_for_ply(ply, white_pk, black_pk);
                        match recorder.record_with_retry(ply, mv, fen, mover) {
                            Ok((sig, resp_endpoint)) => {
                                let endpoint = if resp_endpoint.is_empty() {
                                    &er_endpoint
                                } else {
                                    &resp_endpoint
                                };
                                info!(
                                        "[ER] Move {} for game {} delegated & recorded on Ephemeral Rollup, sig {} — inspect: {}",
                                        mv, gid, sig, er_explorer_url_for(endpoint, &sig)
                                    )
                            }
                            Err(e) => {
                                error!("[VPS] record_move failed {} game {}: {}", mv, gid, e)
                            }
                        }
                    }
                    let _ = flush_tx.send(());
                });
            }
            RollupEvent::BatchFailed { game_id, .. } | RollupEvent::NeedResync { game_id } => {
                send_network_msg(
                    &network_state,
                    NetworkMessage::ResyncRequest { game_id: *game_id },
                );
                warn!("Requested resync for game {}", game_id);
            }
            _ => {}
        }
    }
}

pub(super) fn handle_network_to_rollup_events(
    mut network_events: MessageReader<NetworkEvent>,
    network_state: Res<OnlineNetworkState>,
    mut rollup_events: MessageWriter<RollupEvent>,
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut bridge: ResMut<RollupNetworkBridge>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
) {
    let er_endpoint = magicblock_resolver.er_endpoint().to_string();
    let white_black = resolve_white_black(rollup_manager.is_creator, solana_state.as_deref());
    let me = solana_state.as_deref().and_then(|s| s.wallet_pubkey);
    for event in network_events.read() {
        let msg = match event {
            NetworkEvent::MessageReceived(m) => m,
            _ => continue,
        };

        match msg {
            NetworkMessage::BatchPropose {
                game_id,
                start_turn,
                moves,
                next_fens,
            } => {
                let incoming_hash = calculate_batch_hash(
                    *game_id,
                    *start_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                if bridge.sent_batch_hashes.contains(&incoming_hash) {
                    continue;
                }

                if !validate_batch_proposal(
                    *start_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                    &rollup_manager,
                ) {
                    warn!("Rejected invalid BatchPropose for game {}", game_id);
                    rollup_events.write(RollupEvent::BatchFailed {
                        game_id: *game_id,
                        moves: moves.clone(),
                        next_fens: next_fens.clone(),
                    });
                    continue;
                }

                let batch_hash = calculate_batch_hash(
                    *game_id,
                    *start_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchAccept {
                        game_id: *game_id,
                        batch_hash,
                    },
                );

                info!(
                    "Peer batch validated for game {} — peer will submit via record_move",
                    game_id
                );
            }

            NetworkMessage::BatchAccept {
                game_id,
                batch_hash,
            } => {
                info!(
                    "Peer accepted batch for game {}, hash: {}",
                    game_id, batch_hash
                );
                if bridge.last_sent_batch_hash.as_deref() == Some(batch_hash.as_str()) {
                    bridge.awaiting_commit_confirmation = false;
                }
                // Submit the accepted batch via VPS record_move on the ER.
                if let Some((moves, next_fens)) = bridge.pending_batches.remove(batch_hash.as_str())
                {
                    let Some((white_pk, black_pk)) = white_black else {
                        warn!(
                            "[VPS] Accepted batch for game {} dropped — no wallet state to attribute movers",
                            game_id
                        );
                        continue;
                    };
                    let gid = *game_id;
                    let base_nonce = bridge.move_nonce;
                    bridge.move_nonce += moves.len() as u64;
                    let er_endpoint = er_endpoint.clone();
                    crate::core::net_task::spawn_detached("bridge", move || {
                        use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
                        let mut recorder = MoveRecorder::new(gid, me);
                        for (i, (mv, fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
                            let ply = base_nonce + i as u64;
                            let mover = mover_wallet_for_ply(ply, white_pk, black_pk);
                            match recorder.record(ply, mv, fen, mover) {
                                Ok((sig, resp_endpoint)) => {
                                    let endpoint = if resp_endpoint.is_empty() {
                                        &er_endpoint
                                    } else {
                                        &resp_endpoint
                                    };
                                    info!(
                                        "[ER] Move {} for game {} delegated & recorded on Ephemeral Rollup, sig {} — inspect: {}",
                                        mv, gid, sig, er_explorer_url_for(endpoint, &sig)
                                    )
                                }
                                Err(e) => {
                                    error!("[VPS] record_move failed {} game {}: {}", mv, gid, e)
                                }
                            }
                        }
                    });
                }
            }

            NetworkMessage::BatchReject { game_id, reason } => {
                warn!("Peer rejected batch for game {}: {}", game_id, reason);
                send_network_msg(
                    &network_state,
                    NetworkMessage::ResyncRequest { game_id: *game_id },
                );
            }

            NetworkMessage::Committed {
                game_id,
                tx_sig,
                new_fen,
                new_turn,
            } => {
                // Peer-reported, not read from the Game PDA: only a newer,
                // non-conflicting position is accepted.
                if rollup_manager.accept_peer_baseline(*game_id, new_fen, *new_turn) {
                    info!("Batch committed on-chain, tx: {}", tx_sig);
                    rollup_events.write(RollupEvent::BatchCommitted {
                        game_id: *game_id,
                        new_fen: new_fen.clone(),
                        new_turn: *new_turn,
                    });
                }
            }

            NetworkMessage::ResyncRequest { game_id } => {
                if *game_id == rollup_manager.game_id {
                    send_network_msg(
                        &network_state,
                        NetworkMessage::ResyncResponse {
                            game_id: *game_id,
                            committed_fen: rollup_manager.committed_fen.clone(),
                            committed_turn: rollup_manager.committed_turn,
                        },
                    );
                }
            }

            NetworkMessage::ResyncResponse {
                game_id,
                committed_fen,
                committed_turn,
            } => {
                if rollup_manager.accept_peer_baseline(*game_id, committed_fen, *committed_turn) {
                    info!(
                        "Resynced game {} from peer, turn {}",
                        game_id, committed_turn
                    );
                }
            }

            NetworkMessage::Move { .. } => {
                // Only local moves belong in pending_batch; the sync layer handles remote broadcasts.
            }

            // Replay moves after the last version supplied in BraidResyncRequest.
            NetworkMessage::BraidResyncRequest {
                game_id,
                since_version,
            } => {
                let gid = *game_id;
                let since = since_version.clone();
                let msg_tx = network_state.message_sender.clone();

                crate::core::net_task::spawn_detached("bridge", move || {
                    use crate::multiplayer::vps_client;
                    use braid_chess::MovePayload;

                    // Fetch the move log from the VPS (authoritative archive).
                    // Falls back to an empty list if unavailable.
                    let all_moves: Vec<MovePayload> =
                        vps_client::fetch_move_log(gid).unwrap_or_default();

                    let since_ver = since.clone();
                    let missed: Vec<String> = all_moves
                        .iter()
                        .skip_while(|m| {
                            braid_chess::version_hash(&m.fen_after, m.move_number) != since_ver
                        })
                        .skip(1) // skip the matching entry itself
                        .filter_map(|m| serde_json::to_string(m).ok())
                        .collect();

                    if missed.is_empty() {
                        info!("[RESYNC] No missed moves for game {} since {}", gid, since);
                        return;
                    }

                    info!(
                        "[RESYNC] Sending {} missed moves for game {} since {}",
                        missed.len(),
                        gid,
                        since
                    );
                    if let Some(tx) = msg_tx {
                        let _ = tx.send(NetworkMessage::BraidResyncResponse {
                            game_id: gid,
                            move_payloads: missed,
                        });
                    }
                });
            }

            // A peer sent us missed moves in response to our BraidResyncRequest.
            // Replay each one through the normal NetworkEvent path.
            NetworkMessage::BraidResyncResponse {
                game_id,
                move_payloads,
            } => {
                use braid_chess::MovePayload;
                let gid = *game_id;
                info!(
                    "[RESYNC] Received {} missed moves for game {}",
                    move_payloads.len(),
                    gid
                );
                for json in move_payloads {
                    if let Ok(p) = serde_json::from_str::<MovePayload>(json) {
                        rollup_events.write(RollupEvent::ResyncedMove {
                            game_id: gid,
                            move_uci: p.uci.clone(),
                            next_fen: p.fen_after.clone(),
                            move_number: p.move_number,
                        });
                    }
                }
            }

            // Apply peer snapshots when spectating or catching up on missed moves.
            NetworkMessage::GameSnapshot {
                game_id,
                fen,
                move_payloads,
                head_version,
            } => {
                use braid_chess::MovePayload;
                let gid = *game_id;
                if gid != rollup_manager.game_id {
                    // Not our game — ignore.
                } else {
                    info!(
                        "[SNAPSHOT] Received game snapshot for {} ({} moves, head {})",
                        gid,
                        move_payloads.len(),
                        head_version
                    );
                    // Emit a full-state resync event so the game layer can
                    // reconstruct position from the authoritative FEN.
                    rollup_events.write(RollupEvent::SnapshotReceived {
                        game_id: gid,
                        fen: fen.clone(),
                        move_payloads: move_payloads
                            .iter()
                            .filter_map(|j| serde_json::from_str::<MovePayload>(j).ok())
                            .collect(),
                        head_version: head_version.clone(),
                    });
                }
            }

            _ => {}
        }
    }
}

pub(super) fn process_batch_commit_requests(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut _rollup_events: MessageWriter<RollupEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
    mut recent_txs: ResMut<RecentTransactions>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
) {
    if bridge.awaiting_commit_confirmation {
        return;
    }
    // Never record from a counter that is about to be replaced by the chain's
    // nonce (`request_nonce_resync`); the batch stays pending until it lands.
    if bridge.nonce_rx.is_some() {
        return;
    }
    if rollup_manager.status != GameStateStatus::Pending || !rollup_manager.should_flush() {
        return;
    }

    let Some((white_pk, black_pk)) =
        resolve_white_black(rollup_manager.is_creator, solana_state.as_deref())
    else {
        warn!(
            "[VPS] Batch flush for game {} skipped — no wallet state to attribute movers",
            rollup_manager.game_id
        );
        return;
    };

    if let Some((moves, next_fens)) = rollup_manager.prepare_batch_for_commit() {
        let base_nonce = bridge.move_nonce;
        let outcome = submit_moves_via_vps(
            rollup_manager.game_id,
            &moves,
            &next_fens,
            base_nonce,
            white_pk,
            black_pk,
            solana_state.as_deref().and_then(|s| s.wallet_pubkey),
            &mut magicblock_events,
            &mut recent_txs,
            magicblock_resolver.er_endpoint(),
        );
        bridge.move_nonce = base_nonce.saturating_add(outcome.recorded as u64);

        if outcome.recorded > 0 {
            let final_fen = next_fens
                .get(outcome.recorded - 1)
                .cloned()
                .unwrap_or_else(|| rollup_manager.committed_fen.clone());
            rollup_manager.batch_commit_success(final_fen, outcome.recorded);
        }

        if outcome.recorded < moves.len() {
            let remaining_moves = moves[outcome.recorded..].to_vec();
            let remaining_fens = next_fens[outcome.recorded..].to_vec();
            rollup_manager.batch_commit_failed(remaining_moves, remaining_fens);
            bridge.awaiting_commit_confirmation = false;
            if let Some(error) = outcome.error {
                error!(
                    "[VPS] Batch flush for game {} stopped after {} / {} move(s): {}",
                    rollup_manager.game_id,
                    outcome.recorded,
                    moves.len(),
                    error
                );
                crate::multiplayer::network::vps::emit_client_event(
                    crate::multiplayer::network::vps::ClientEvent::new("solana_er_record_failed")
                        .game_id(rollup_manager.game_id)
                        .reason(error),
                );
            }
        } else {
            bridge.awaiting_commit_confirmation = false;
        }
    }
}

pub(super) fn validate_batch_proposal(
    start_turn: u16,
    moves: &[String],
    next_fens: &[String],
    rollup_manager: &EphemeralRollupManager,
) -> bool {
    if start_turn != rollup_manager.committed_turn {
        warn!(
            "Batch start_turn {} != committed_turn {}",
            start_turn, rollup_manager.committed_turn
        );
        return false;
    }
    !moves.is_empty() && moves.len() == next_fens.len()
}

#[derive(Debug, Default)]
pub(super) struct BatchSubmitOutcome {
    pub(super) recorded: usize,
    pub(super) error: Option<String>,
}

pub(super) fn submit_moves_via_vps(
    game_id: u64,
    moves: &[String],
    next_fens: &[String],
    base_nonce: u64,
    white_pk: Pubkey,
    black_pk: Pubkey,
    me: Option<Pubkey>,
    magicblock_events: &mut MessageWriter<MagicBlockEvent>,
    recent_txs: &mut RecentTransactions,
    fallback_er_endpoint: &str,
) -> BatchSubmitOutcome {
    use crate::multiplayer::rollup::magicblock::er_explorer_url_for;

    let mut outcome = BatchSubmitOutcome::default();
    let mut recorder = MoveRecorder::new(game_id, me);
    for (i, (move_str, next_fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
        let ply = base_nonce + i as u64;
        let mover = mover_wallet_for_ply(ply, white_pk, black_pk);
        match recorder.record(ply, move_str, next_fen, mover) {
            Ok((sig, resp_endpoint)) => {
                let endpoint = if resp_endpoint.is_empty() {
                    fallback_er_endpoint
                } else {
                    resp_endpoint.as_str()
                };
                info!(
                    "[ER] Move {} for game {} delegated & recorded on Ephemeral Rollup, sig {} — inspect: {}",
                    move_str, game_id, sig, er_explorer_url_for(endpoint, &sig)
                );
                recent_txs.push(move_str.clone(), sig.clone());
                magicblock_events.write(MagicBlockEvent::TransactionRoutedToEr { signature: sig });
                outcome.recorded += 1;
            }
            Err(e) => {
                error!(
                    "[VPS] record_move failed for {} game {}: {}",
                    move_str, game_id, e
                );
                outcome.error = Some(e.to_string());
                return outcome;
            }
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_chess::MovePayload;

    const AFTER_E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const AFTER_E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";

    fn log() -> Vec<MovePayload> {
        vec![
            MovePayload::from_uci("e2e4", AFTER_E4, 1, "n").with_auth(Some("sig-e4".into())),
            MovePayload::from_uci("e7e5", AFTER_E5, 1, "n").with_auth(Some("sig-e5".into())),
        ]
    }

    #[test]
    fn a_relayed_move_takes_the_signature_logged_for_that_ply_and_move() {
        let log = log();
        assert_eq!(logged_auth(&log, 1, "e2e4").as_deref(), Some("sig-e4"));
        assert_eq!(logged_auth(&log, 2, "e7e5").as_deref(), Some("sig-e5"));
        assert_eq!(logged_auth(&log, 2, "E7E5").as_deref(), Some("sig-e5"));
    }

    #[test]
    fn a_signature_is_never_taken_for_another_ply_or_move() {
        let log = log();
        assert_eq!(logged_auth(&log, 3, "e7e5"), None);
        assert_eq!(logged_auth(&log, 2, "e2e4"), None);
        assert_eq!(logged_auth(&log, 2, "d7d5"), None);
    }

    #[test]
    fn an_unsigned_log_entry_yields_nothing() {
        let unsigned = vec![MovePayload::from_uci("e2e4", AFTER_E4, 1, "n")];
        assert_eq!(logged_auth(&unsigned, 1, "e2e4"), None);
    }
}
