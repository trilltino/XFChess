use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::game::events::{GameEndedEvent, GameStartedEvent};
use crate::game::replay::ParsedPgnGameResource;
use crate::multiplayer::rollup::magicblock::DelegationStatus;
use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
use crate::multiplayer::{
    calculate_batch_hash, EphemeralRollupManager, GameStateStatus, MagicBlockEvent,
    MagicBlockResolver, NetworkEvent, NetworkMessage, OnlineNetworkState, RollupEvent,
};
use crate::solana::instructions::PROGRAM_ID as SOLANA_PROGRAM_ID;
use crate::ui::menus::game_over_popup::GameOverPayoutInfo;

#[derive(Debug, Default)]
struct FinalizationResult {
    sig: String,
    winner_lamports: u64,
    country_fee: u64,
    operating_cost_lamports: u64,
    elo_fee: u64,
    /// Set only from chain evidence, never from the HTTP response alone.
    status: SettlementStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SettlementStatus {
    /// The finalize signature confirmed, or the Game PDA is closed (which
    /// only `finalize_game` does, after paying out the escrow).
    Confirmed,
    /// Not proven on chain; the settlement worker keeps retrying server-side.
    Pending(String),
}

impl Default for SettlementStatus {
    fn default() -> Self {
        Self::Pending("Settlement not yet confirmed on chain".to_string())
    }
}

/// Combine signature status and Game PDA existence into settlement status.
/// None means an unknown signature result or failed account lookup.
fn settlement_status(
    sig_status: Option<Result<(), String>>,
    game_closed: Option<bool>,
) -> SettlementStatus {
    match (sig_status, game_closed) {
        (Some(Ok(())), _) | (_, Some(true)) => SettlementStatus::Confirmed,
        (Some(Err(e)), _) => SettlementStatus::Pending(format!(
            "Settlement transaction failed ({e}); it will be retried automatically"
        )),
        (None, _) => SettlementStatus::Pending(
            "Settlement submitted; waiting for on-chain confirmation".to_string(),
        ),
    }
}

/// Poll the finalize signature, then fall back to the Game PDA's existence.
fn verify_settlement_on_chain(
    rpc: &RpcClient,
    game_pda: &Pubkey,
    sig: Option<&str>,
) -> SettlementStatus {
    let mut sig_status = None;
    if let Some(sig) = sig.and_then(|s| s.parse::<solana_sdk::signature::Signature>().ok()) {
        for _ in 0..20 {
            match rpc.get_signature_status_with_commitment(&sig, CommitmentConfig::confirmed()) {
                Ok(Some(Ok(()))) => {
                    sig_status = Some(Ok(()));
                    break;
                }
                Ok(Some(Err(e))) => {
                    sig_status = Some(Err(format!("{e:?}")));
                    break;
                }
                Ok(None) | Err(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
            }
        }
    }
    let game_closed = if sig_status == Some(Ok(())) {
        None
    } else {
        rpc.get_account_with_commitment(game_pda, CommitmentConfig::confirmed())
            .ok()
            .map(|resp| resp.value.is_none())
    };
    settlement_status(sig_status, game_closed)
}

const MAX_UNDELEGATE_WAIT_SECS: u64 = 60;

const MAX_JOINER_DELEGATION_WAIT_SECS: u64 = 60;

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

#[derive(Resource, Default)]
pub struct RollupNetworkBridge {
    awaiting_commit_confirmation: bool,
    last_sent_batch_hash: Option<String>,
    pending_batches: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    sent_batch_hashes: std::collections::HashSet<String>,
    pending_delegation_pda: Option<Pubkey>,
    pending_game_id: Option<u64>,
    delegation_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    delegation_retry_cooldown: f32,
    move_nonce: u64,
    pending_finalization: Option<PendingFinalization>,
    finalization_rx: Option<oneshot::Receiver<FinalizationResult>>,
    nonce_rx: Option<oneshot::Receiver<u64>>,
    pgn_rx: Option<oneshot::Receiver<Option<nimzovich_engine::ParsedPgnGame>>>,
    joiner_delegation_wait_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    joiner_delegation_wait_game_id: Option<u64>,
    game_end_moves_flushing: bool,
    game_end_flush_rx: Option<oneshot::Receiver<()>>,
}

const MAX_FINALIZATION_WAIT_FRAMES: u32 = 600;

#[derive(Debug)]
struct PendingFinalization {
    game_id: u64,
    winner: Option<String>,
    local_pk: Pubkey,
    is_creator: bool,
    frames_waited: u32,
    wager_lamports: u64,
}

impl RollupNetworkBridge {
    fn new() -> Self {
        Self {
            move_nonce: 1,
            ..Default::default()
        }
    }

    pub fn has_pending_finalization(&self) -> bool {
        self.pending_finalization.is_some()
            || self.finalization_rx.is_some()
            || self.game_end_moves_flushing
            || self.game_end_flush_rx.is_some()
    }

    /// Next `record_move` nonce, set from an authoritative source (the
    /// chain, via a resumed game's verified state).
    pub fn set_move_nonce(&mut self, next_nonce: u64) {
        self.move_nonce = next_nonce.max(1);
    }

    /// Fetch the Game PDA's nonce and apply it before the next batch, so this
    /// client never records moves from a stale local counter.
    pub fn request_nonce_resync(&mut self, game_id: u64) {
        let (nonce_tx, nonce_rx) = oneshot::channel::<u64>();
        self.nonce_rx = Some(nonce_rx);
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                use crate::multiplayer::vps_client;
                match vps_client::vps_fetch_move_nonce(game_id) {
                    Ok(next_nonce) => {
                        info!(
                            "[NONCE] Resynced nonce for game {} → {}",
                            game_id, next_nonce
                        );
                        let _ = nonce_tx.send(next_nonce);
                    }
                    Err(e) => {
                        warn!(
                            "[NONCE] Failed to fetch nonce for game {}: {} — keeping local nonce",
                            game_id, e
                        );
                    }
                }
            })
            .detach();
    }

    pub fn reset_preserving_finalization(&mut self) {
        let mut fresh = Self::default();
        fresh.pending_finalization = self.pending_finalization.take();
        fresh.finalization_rx = self.finalization_rx.take();
        fresh.game_end_moves_flushing = self.game_end_moves_flushing;
        fresh.game_end_flush_rx = self.game_end_flush_rx.take();
        *self = fresh;
    }
}

pub struct RollupNetworkBridgePlugin;

impl Plugin for RollupNetworkBridgePlugin {
    fn build(&self, app: &mut App) {
        use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;

        app.insert_resource(RollupNetworkBridge::new());

        let mut resolver = MagicBlockResolver::default();
        resolver.set_solana_rpc(Arc::new(RpcClient::new_with_commitment(
            DEVNET_RPC_URL.to_string(),
            CommitmentConfig::confirmed(),
        )));
        app.insert_resource(resolver);

        app.init_resource::<RecentTransactions>();
        app.add_message::<MagicBlockEvent>();

        // Order finalization, batch flush, and retry in one chain so retry observes
        // game_end_moves_flushing before attempting undelegation.
        app.add_systems(
            Update,
            handle_rollup_to_network_events
                .after(crate::multiplayer::systems::finalize_game_on_end),
        );
        app.add_systems(Update, handle_network_to_rollup_events);
        app.add_systems(Update, process_batch_commit_requests);

        // Magic Block ER delegation systems
        app.add_systems(Update, handle_game_start_delegation);
        app.add_systems(Update, retry_pending_delegation);
        app.add_systems(Update, handle_game_end_undelegation);
        app.add_systems(Update, handle_magic_block_events);

        app.add_systems(Update, poll_delegation_tasks);
        app.add_systems(Update, poll_joiner_delegation_wait);
        app.add_systems(Update, poll_game_end_flush);
        app.add_systems(
            Update,
            retry_pending_finalization.after(handle_rollup_to_network_events),
        );

        // Post-finalization: apply payout result to game-over popup resource.
        app.add_systems(Update, apply_finalization_result);
        // Nonce resync: apply on-chain nonce once the async fetch completes.
        app.add_systems(Update, apply_nonce_resync);
        // PGN export: fetch Braid move log and build replay resource after game ends.
        app.add_systems(Update, handle_game_end_pgn_export);
        app.add_systems(Update, apply_pgn_export_result);
        // Discard finished-game causal state to bound memory across long sessions.
        app.add_systems(Update, handle_game_end_causal_cleanup);

        info!("RollupNetworkBridgePlugin initialized with Magic Block ER support");
    }
}

fn send_network_msg(state: &OnlineNetworkState, msg: NetworkMessage) {
    if let Some(tx) = &state.message_sender {
        if let Err(e) = tx.send(msg) {
            warn!("Failed to send NetworkMessage: {}", e);
        }
    }
}

fn resolve_white_black(
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

fn mover_wallet_for_ply(nonce: u64, white: Pubkey, black: Pubkey) -> Pubkey {
    if nonce % 2 == 1 {
        white
    } else {
        black
    }
}

fn handle_rollup_to_network_events(
    mut rollup_events: MessageReader<RollupEvent>,
    network_state: Res<OnlineNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    rollup_manager: Res<EphemeralRollupManager>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
) {
    let er_endpoint = magicblock_resolver.er_endpoint().to_string();
    let white_black = resolve_white_black(rollup_manager.is_creator, solana_state.as_deref());
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
                bevy::tasks::IoTaskPool::get()
                    .spawn(async move {
                        use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
                        use crate::multiplayer::vps_client;
                        for (i, (mv, fen)) in moves_owned.iter().zip(fens_owned.iter()).enumerate()
                        {
                            let ply = base_nonce + i as u64;
                            let mover = mover_wallet_for_ply(ply, white_pk, black_pk).to_string();
                            match vps_client::record_move(gid, mv, fen, ply, &mover) {
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
                    })
                    .detach();
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

fn handle_network_to_rollup_events(
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
                    bevy::tasks::IoTaskPool::get()
                        .spawn(async move {
                            use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
                            use crate::multiplayer::vps_client;
                            for (i, (mv, fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
                                let ply = base_nonce + i as u64;
                                let mover = mover_wallet_for_ply(ply, white_pk, black_pk).to_string();
                                match vps_client::record_move(gid, mv, fen, ply, &mover) {
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
                                    Err(e) => error!(
                                        "[VPS] record_move failed {} game {}: {}",
                                        mv, gid, e
                                    ),
                                }
                            }
                        })
                        .detach();
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

                bevy::tasks::IoTaskPool::get()
                    .spawn(async move {
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
                    })
                    .detach();
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

fn process_batch_commit_requests(
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

fn validate_batch_proposal(
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
struct BatchSubmitOutcome {
    recorded: usize,
    error: Option<String>,
}

fn submit_moves_via_vps(
    game_id: u64,
    moves: &[String],
    next_fens: &[String],
    base_nonce: u64,
    white_pk: Pubkey,
    black_pk: Pubkey,
    magicblock_events: &mut MessageWriter<MagicBlockEvent>,
    recent_txs: &mut RecentTransactions,
    fallback_er_endpoint: &str,
) -> BatchSubmitOutcome {
    use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
    use crate::multiplayer::vps_client;

    let mut outcome = BatchSubmitOutcome::default();
    for (i, (move_str, next_fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
        let ply = base_nonce + i as u64;
        let mover = mover_wallet_for_ply(ply, white_pk, black_pk).to_string();
        match vps_client::record_move(game_id, move_str, next_fen, ply, &mover) {
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

fn handle_game_start_delegation(
    mut game_started_events: MessageReader<GameStartedEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
) {
    for event in game_started_events.read() {
        // Use rollup_manager’s on-chain game ID; event.game_id identifies the gossip session.

        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            warn!(
                "[DELEGATION] rollup_manager.game_id is 0 at GameStarted (p2p id {}); deferring",
                event.game_id
            );
            continue;
        };

        // Only white delegates; concurrent delegation changes PDA ownership and fails the second transaction.
        if !rollup_manager.is_creator {
            // The joiner polls Game PDA ownership to observe delegation because the
            // host's local delegation task cannot update the joiner's resolver.
            if magicblock_resolver.is_delegated()
                || bridge.joiner_delegation_wait_game_id == Some(game_id)
            {
                continue;
            }

            let rpc_client = match magicblock_resolver.solana_rpc.clone() {
                Some(client) => client,
                None => {
                    error!("[DELEGATION] No Solana RPC client configured (joiner wait)");
                    continue;
                }
            };

            info!(
                "[DELEGATION] Game {} — joiner does not delegate; waiting to observe creator's delegation",
                game_id
            );

            let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
            let game_pda =
                Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

            bridge.joiner_delegation_wait_game_id = Some(game_id);
            let (tx, rx) = oneshot::channel();
            bridge.joiner_delegation_wait_rx = Some(rx);

            bevy::tasks::IoTaskPool::get()
                .spawn(async move {
                    let result = wait_for_delegation(game_pda, game_id, rpc_client).await;
                    let _ = tx.send(result);
                })
                .detach();

            continue;
        }

        info!(
            "[DELEGATION] Game {} started - spawning ER delegation task",
            game_id
        );

        // Derive the game PDA using the Solana game_id
        let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

        // Need wallet pubkey to satisfy on-chain payer == game.white || game.black check
        let wallet_pubkey = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
            Some(pk) => pk,
            None => {
                warn!(
                    "[DELEGATION] No wallet pubkey for game {} — deferring",
                    game_id
                );
                bridge.pending_delegation_pda = Some(game_pda);
                bridge.pending_game_id = Some(game_id);
                continue;
            }
        };

        let rpc_client = match magicblock_resolver.solana_rpc.clone() {
            Some(client) => client,
            None => {
                error!("[DELEGATION] No Solana RPC client configured");
                bridge.pending_delegation_pda = Some(game_pda);
                bridge.pending_game_id = Some(game_id);
                continue;
            }
        };

        // Choose the signer from this game's used_global_session, not the wallet's
        // current session flag. Per-game sessions delegate through the VPS.
        let global_session_keypair_bytes = solana_state
            .as_ref()
            .filter(|_| rollup_manager.used_global_session)
            .and_then(|s| s.global_session_keypair.as_ref())
            .map(|kp| kp.to_bytes().to_vec());
        let _ = wallet_pubkey; // only used above to gate readiness

        // Keep the pending game PDA on all failures so delegation can emit an error
        // and retry. Clear it only after confirmed success.
        bridge.pending_delegation_pda = Some(game_pda);
        bridge.pending_game_id = Some(game_id);

        let (tx, rx) = oneshot::channel();
        bridge.delegation_rx = Some(rx);

        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                let result = spawn_delegation_task(
                    game_pda,
                    game_id,
                    rpc_client,
                    global_session_keypair_bytes,
                )
                .await;
                let _ = tx.send(result);
            })
            .detach();

        // Item 5: fetch on-chain nonce so we never start with a stale local nonce.
        bridge.request_nonce_resync(game_id);
    }
}

async fn spawn_delegation_task(
    game_pda: Pubkey,
    game_id: u64,
    rpc_client: Arc<RpcClient>,
    global_session_keypair_bytes: Option<Vec<u8>>,
) -> Result<Pubkey, String> {
    use solana_sdk::signer::Signer;

    info!(
        "[DELEGATION-TASK] Starting delegation for game {} (PDA: {})",
        game_id, game_pda
    );

    let mut resolver = crate::multiplayer::rollup::magicblock::MagicBlockResolver::default();
    resolver.set_solana_rpc(rpc_client.clone());
    resolver.set_game_id(game_id);

    if let Some(kp_bytes) = global_session_keypair_bytes {
        // The global session key stored in game.fee_payer satisfies both payer
        // and fee_payer locally, without a wallet round trip.
        let session_kp = solana_sdk::signature::Keypair::try_from(kp_bytes.as_slice())
            .map_err(|e| format!("session keypair: {e}"))?;
        let ix = resolver
            .create_delegation_instruction(game_pda, session_kp.pubkey(), session_kp.pubkey())
            .map_err(|e| format!("build delegation ix: {}", e))?;
        // Use fast submit-and-poll to avoid an extra preflight RPC round trip.
        use crate::multiplayer::solana::submit::{submit_local_tx, SubmitConfig};
        return match submit_local_tx(&rpc_client, &session_kp, &[ix], SubmitConfig::fast()) {
            Ok(sig) => {
                info!(
                    "[DELEGATION-TASK] SUCCESS for game {} sig: {} (session-signed, no wallet popup)",
                    game_id, sig
                );
                Ok(game_pda)
            }
            Err(e) => {
                error!("[DELEGATION-TASK] FAILED for game {}: {}", game_id, e);
                Err(e)
            }
        };
    }

    // The VPS holds the per-game fee payer key and must sign delegation for that flow.
    match crate::multiplayer::vps_client::vps_delegate_game(game_id) {
        Ok(sig) => {
            info!(
                "[DELEGATION-TASK] SUCCESS for game {} sig: {} (VPS-signed, no wallet popup)",
                game_id, sig
            );
            Ok(game_pda)
        }
        Err(e) => {
            error!("[DELEGATION-TASK] FAILED for game {}: {}", game_id, e);
            Err(e)
        }
    }
}

async fn wait_for_delegation(
    game_pda: Pubkey,
    game_id: u64,
    rpc_client: Arc<RpcClient>,
) -> Result<Pubkey, String> {
    use crate::multiplayer::rollup::magicblock::DELEGATION_PROGRAM_ID;

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|_| "bad delegation program id".to_string())?;

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(MAX_JOINER_DELEGATION_WAIT_SECS);
    loop {
        match rpc_client.get_account(&game_pda) {
            Ok(acc) if acc.owner == delegation_program_id => {
                info!(
                    "[DELEGATION] Game {} observed delegated (joiner) — PDA owner is now the delegation program",
                    game_id
                );
                return Ok(game_pda);
            }
            Ok(_) => {}  // not delegated yet
            Err(_) => {} // transient RPC error — keep polling
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "game {} not observed delegated after {}s",
                game_id, MAX_JOINER_DELEGATION_WAIT_SECS
            ));
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn poll_joiner_delegation_wait(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if let Some(ref mut rx) = bridge.joiner_delegation_wait_rx {
        match rx.try_recv() {
            Ok(Ok(game_pda)) => {
                info!("Delegation observed by joiner for game {}", game_pda);
                magicblock_resolver.delegation_status = DelegationStatus::Delegated;
                magicblock_resolver.delegated_game_pda = Some(game_pda);
                magicblock_events.write(MagicBlockEvent::GameDelegated { game_pda });
                // The joiner (and a resumed client) records moves too; start
                // from the chain's nonce, not a local counter that may be 0.
                if let Some(game_id) = bridge.joiner_delegation_wait_game_id {
                    bridge.request_nonce_resync(game_id);
                }
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
            Ok(Err(e)) => {
                error!("[DELEGATION] Joiner delegation wait failed: {}", e);
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still waiting, nothing to do.
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                error!("[DELEGATION] Joiner delegation wait task dropped");
                bridge.joiner_delegation_wait_rx = None;
                bridge.joiner_delegation_wait_game_id = None;
            }
        }
    }
}

fn poll_game_end_flush(mut bridge: ResMut<RollupNetworkBridge>) {
    if let Some(ref mut rx) = bridge.game_end_flush_rx {
        match rx.try_recv() {
            Ok(()) | Err(oneshot::error::TryRecvError::Closed) => {
                bridge.game_end_moves_flushing = false;
                bridge.game_end_flush_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still flushing, nothing to do.
            }
        }
    }
}

fn poll_delegation_tasks(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if let Some(ref mut rx) = bridge.delegation_rx {
        match rx.try_recv() {
            Ok(Ok(game_pda)) => {
                info!("Delegation completed for game {}", game_pda);
                magicblock_resolver.delegation_status = DelegationStatus::Delegated;
                magicblock_resolver.delegated_game_pda = Some(game_pda);
                magicblock_events.write(MagicBlockEvent::GameDelegated { game_pda });
                bridge.delegation_rx = None;
                // Clear pending identifiers only on confirmed success so failures can be retried.
                bridge.pending_delegation_pda = None;
                bridge.pending_game_id = None;
            }
            Ok(Err(e)) => {
                error!("Delegation failed: {}", e);
                if let Some(pda) = bridge.pending_delegation_pda {
                    magicblock_events.write(MagicBlockEvent::DelegationFailed {
                        game_pda: pda,
                        error: e,
                    });
                }
                bridge.delegation_rx = None;
                bridge.delegation_retry_cooldown = 30.0;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Task still running, nothing to do
            }
            Err(_) => {
                error!("Delegation task dropped");
                if let Some(pda) = bridge.pending_delegation_pda {
                    magicblock_events.write(MagicBlockEvent::DelegationFailed {
                        game_pda: pda,
                        error: "delegation task dropped before completing".to_string(),
                    });
                }
                bridge.delegation_rx = None;
                bridge.delegation_retry_cooldown = 30.0;
            }
        }
    }
}

fn retry_pending_delegation(
    mut bridge: ResMut<RollupNetworkBridge>,
    time: Res<Time>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
    magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if bridge.delegation_rx.is_some() {
        return;
    }

    // Back off after signing or broadcast failures to avoid retrying on every frame.
    if bridge.delegation_retry_cooldown > 0.0 {
        bridge.delegation_retry_cooldown -= time.delta_secs();
        return;
    }

    let game_pda = match bridge.pending_delegation_pda {
        Some(pda) => pda,
        None => return,
    };

    let game_id = match bridge.pending_game_id {
        Some(id) => id,
        None => return,
    };

    let wallet_pubkey = match solana_state.as_ref().and_then(|s| s.wallet_pubkey) {
        Some(pk) => pk,
        None => return, // wallet not ready yet; try next frame
    };

    let rpc_client = match magicblock_resolver.solana_rpc.clone() {
        Some(client) => client,
        None => {
            error!("No Solana RPC client configured for retry delegation");
            return;
        }
    };

    // Retain pending game identifiers until confirmed success so another retry can find the game.

    // Same per-game gating as `handle_game_start_delegation` — see its
    // comment for why this can't be the live `global_session_active` flag.
    let global_session_keypair_bytes = solana_state
        .as_ref()
        .filter(|_| rollup_manager.used_global_session)
        .and_then(|s| s.global_session_keypair.as_ref())
        .map(|kp| kp.to_bytes().to_vec());
    let _ = wallet_pubkey; // only used above to gate readiness

    let (tx, rx) = oneshot::channel();
    bridge.delegation_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result =
                spawn_delegation_task(game_pda, game_id, rpc_client, global_session_keypair_bytes)
                    .await;
            let _ = tx.send(result);
        })
        .detach();

    info!(
        "Retry delegation spawned for game {} PDA {}",
        game_id, game_pda
    );

    let _ = magicblock_events; // suppress unused warning
}

fn handle_game_end_causal_cleanup(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    mut causal: ResMut<crate::multiplayer::types::CausalChainState>,
) {
    for event in game_ended_events.read() {
        let game_id = event.game_id;
        causal.last_seq.retain(|(gid, _), _| *gid != game_id);
        causal.head_version.retain(|(gid, _), _| *gid != game_id);
        causal.roster.remove(&game_id);
    }
}

fn handle_game_end_undelegation(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
) {
    for event in game_ended_events.read() {
        // Only the host drives settlement; the backend worker recovers if it dies mid-flow.
        if !rollup_manager.is_creator {
            continue;
        }
        // Use the Solana on-chain game_id (rollup_manager), not the P2P event ID.
        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            event.game_id
        };

        info!(
            "[FINALIZE] Game {} ended (winner={:?} reason={}) — preparing on-chain finalization",
            game_id, event.winner, event.reason
        );

        // Derive and log the move_log PDA so the user can look up moves on Solscan.
        let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let move_log_pda = solana_sdk::pubkey::Pubkey::find_program_address(
            &[b"move_log", &game_id.to_le_bytes()],
            &program_id,
        )
        .0;
        info!("[FINALIZE] move_log PDA: {}", move_log_pda);
        info!(
            "[FINALIZE] Solscan: https://solscan.io/account/{}?cluster=devnet",
            move_log_pda
        );

        let is_delegated = magicblock_resolver.is_delegated();
        let game_pda = magicblock_resolver.get_delegated_game().unwrap_or_default();

        // Resolve white/black wallet pubkeys.
        // is_creator ↔ white; joiner ↔ black.
        let (white_pk, black_pk) = match solana_state.as_ref() {
            Some(s) => {
                let local = s.wallet_pubkey.unwrap_or_default();
                let opponent = s.opponent_pubkey.unwrap_or_default();
                if rollup_manager.is_creator {
                    (local, opponent)
                } else {
                    (opponent, local)
                }
            }
            None => {
                warn!(
                    "[FINALIZE] No wallet state — cannot finalize game {}",
                    game_id
                );
                if is_delegated {
                    magicblock_events.write(MagicBlockEvent::UndelegationFailed {
                        game_pda,
                        error: "no wallet state for finalization".to_string(),
                    });
                }
                continue;
            }
        };

        if white_pk == Pubkey::default() || black_pk == Pubkey::default() {
            warn!(
                "[FINALIZE] Opponent pubkey unavailable for game {} — deferring finalization",
                game_id
            );
            let local_pk = solana_state
                .as_ref()
                .and_then(|s| s.wallet_pubkey)
                .unwrap_or_default();
            let wager = competitive.as_ref().map(|c| c.stake_amount).unwrap_or(0);
            bridge.pending_finalization = Some(PendingFinalization {
                game_id,
                winner: event.winner.clone(),
                local_pk,
                is_creator: rollup_manager.is_creator,
                frames_waited: 0,
                wager_lamports: wager,
            });
            continue;
        }

        let winner = event.winner.clone();

        // Item 4: Free Rated path — game was never delegated, so just update ELO.
        if !is_delegated {
            let w = white_pk.to_string();
            let b = black_pk.to_string();
            let win = winner.clone();
            bevy::tasks::IoTaskPool::get()
                .spawn(async move {
                    use crate::multiplayer::vps_client;
                    if let Err(e) =
                        vps_client::vps_submit_free_rated_result(game_id, win.as_deref(), &w, &b)
                    {
                        error!("[FREE_RATED] ELO update failed for game {}: {e}", game_id);
                    } else {
                        info!("[FREE_RATED] ELO updated for game {}", game_id);
                    }
                })
                .detach();
            continue;
        }

        let wager = competitive.as_ref().map(|c| c.wager_lamports).unwrap_or(0);

        // Defer finalization at least one frame so batch-flush systems have run
        // before checking game_end_moves_flushing.
        let local_pk = if rollup_manager.is_creator {
            white_pk
        } else {
            black_pk
        };
        bridge.pending_finalization = Some(PendingFinalization {
            game_id,
            winner,
            local_pk,
            is_creator: rollup_manager.is_creator,
            frames_waited: 0,
            wager_lamports: wager,
        });
    }
}

fn spawn_finalization_task(
    game_id: u64,
    winner: Option<String>,
    white_pk: Pubkey,
    black_pk: Pubkey,
    wager_lamports: u64,
    result_tx: oneshot::Sender<FinalizationResult>,
    fallback_er_endpoint: String,
) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
            use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
            use crate::multiplayer::vps_client;
            use crate::solana::instructions::PROGRAM_ID as SOLANA_PROGRAM_ID;
            use solana_client::rpc_client::RpcClient;
            use solana_commitment_config::CommitmentConfig;

            // Allow ER move commits to settle before undelegation; these paths have no
            // shared completion signal. Shorter delays require live verification.
            std::thread::sleep(std::time::Duration::from_secs(2));

            match vps_client::vps_undelegate_game(game_id) {
                Ok((sig, resp_endpoint)) => {
                    let endpoint = if resp_endpoint.is_empty() {
                        fallback_er_endpoint.as_str()
                    } else {
                        resp_endpoint.as_str()
                    };
                    info!(
                        "[UNDELEGATE] ER committed for game {} sig {} — inspect: {}",
                        game_id,
                        sig,
                        er_explorer_url_for(endpoint, &sig)
                    )
                }
                Err(e) => error!(
                    "[UNDELEGATE] Failed for game {}: {e} — continuing to finalize",
                    game_id
                ),
            }

            // Item 2: Poll devnet until game PDA owner returns to the program (not ER).
            let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
            let game_pda =
                Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;
            let rpc = RpcClient::new_with_commitment(
                DEVNET_RPC_URL.to_string(),
                CommitmentConfig::confirmed(),
            );
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(MAX_UNDELEGATE_WAIT_SECS);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                match rpc.get_account(&game_pda) {
                    Ok(acc) if acc.owner == program_id => {
                        info!(
                            "[UNDELEGATE] Game {} PDA returned to devnet — proceeding to finalize",
                            game_id
                        );
                        break;
                    }
                    Ok(_) => {}  // still owned by ER
                    Err(_) => {} // transient RPC error — keep polling
                }
                if std::time::Instant::now() >= deadline {
                    warn!(
                        "[UNDELEGATE] Game {} PDA did not return after {}s — finalizing anyway",
                        game_id, MAX_UNDELEGATE_WAIT_SECS
                    );
                    break;
                }
            }

            let w_str = white_pk.to_string();
            let b_str = black_pk.to_string();
            let win_ref = winner.as_deref();

            match vps_client::vps_finalize_game(game_id, win_ref, &w_str, &b_str, wager_lamports) {
                Ok(result) => {
                    let status = verify_settlement_on_chain(&rpc, &game_pda, Some(&result.sig));
                    info!(
                        "[FINALIZED] Game {} finalize returned payout {} lamports, sig {}, chain status {:?}",
                        game_id, result.winner_lamports, result.sig, status
                    );
                    if result.country_fee > 0 {
                        info!(
                            "[TREASURY] Game {} platform fee: {} lamports paid to treasury_vault",
                            game_id, result.country_fee
                        );
                    }
                    let _ = result_tx.send(FinalizationResult {
                        sig: result.sig,
                        winner_lamports: result.winner_lamports,
                        country_fee: result.country_fee,
                        operating_cost_lamports: result.operating_cost_lamports,
                        elo_fee: result.elo_fee,
                        status,
                    });
                }
                Err(e) => {
                    error!("[FINALIZE] Game {} finalization failed: {e}", game_id);
                    // A failed request may still have settled; the worker retries server-side.
                    // Report the observed chain state without claiming an unconfirmed payout.
                    let status = match verify_settlement_on_chain(&rpc, &game_pda, None) {
                        SettlementStatus::Confirmed => SettlementStatus::Confirmed,
                        SettlementStatus::Pending(_) => SettlementStatus::Pending(
                            "Settlement has not completed yet; it will be retried automatically"
                                .to_string(),
                        ),
                    };
                    let _ = result_tx.send(FinalizationResult {
                        status,
                        ..Default::default()
                    });
                }
            }
        })
        .detach();
}

fn retry_pending_finalization(
    mut bridge: ResMut<RollupNetworkBridge>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    magicblock_resolver: Res<MagicBlockResolver>,
) {
    let pending = match bridge.pending_finalization.take() {
        Some(p) => p,
        None => return,
    };

    let opponent_pk = solana_state.as_ref().and_then(|s| s.opponent_pubkey);
    let (white_pk, black_pk) = match opponent_pk {
        Some(opp) => {
            if pending.is_creator {
                (pending.local_pk, opp)
            } else {
                (opp, pending.local_pk)
            }
        }
        None => {
            let new_frames = pending.frames_waited + 1;
            if new_frames > MAX_FINALIZATION_WAIT_FRAMES {
                warn!(
                    "[FINALIZE] Opponent pubkey not received after {} frames for game {} — giving up",
                    new_frames, pending.game_id
                );
                return;
            }
            bridge.pending_finalization = Some(PendingFinalization {
                frames_waited: new_frames,
                ..pending
            });
            return;
        }
    };

    if white_pk == Pubkey::default() || black_pk == Pubkey::default() {
        warn!(
            "[FINALIZE] Resolved pubkeys still default for game {} — skipping",
            pending.game_id
        );
        return;
    }

    // Wait for the final move batch before undelegating; share the existing frame timeout.
    if bridge.game_end_moves_flushing {
        let new_frames = pending.frames_waited + 1;
        if new_frames > MAX_FINALIZATION_WAIT_FRAMES {
            warn!(
                "[FINALIZE] Move batch still flushing after {} frames for game {} — finalizing anyway",
                new_frames, pending.game_id
            );
        } else {
            bridge.pending_finalization = Some(PendingFinalization {
                frames_waited: new_frames,
                ..pending
            });
            return;
        }
    }

    info!(
        "[FINALIZE] Opponent pubkey arrived after {} frames for game {} — finalizing",
        pending.frames_waited, pending.game_id
    );
    let (fin_tx, fin_rx) = oneshot::channel::<FinalizationResult>();
    bridge.finalization_rx = Some(fin_rx);
    spawn_finalization_task(
        pending.game_id,
        pending.winner,
        white_pk,
        black_pk,
        pending.wager_lamports,
        fin_tx,
        magicblock_resolver.er_endpoint().to_string(),
    );
}

fn apply_finalization_result(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut payout_info: Option<ResMut<GameOverPayoutInfo>>,
) {
    let rx = match bridge.finalization_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(result) => {
            bridge.finalization_rx = None;
            if let Some(ref mut info) = payout_info {
                if let SettlementStatus::Pending(reason) = result.status {
                    // Keep the popup in its "settling" state with the reason;
                    // the HTTP response alone never marks a payout complete.
                    warn!("[FINALIZE] Settlement not confirmed on chain: {reason}");
                    info.settlement_pending_reason = Some(reason);
                    if !result.sig.is_empty() {
                        info.finalize_sig = Some(result.sig);
                    }
                    return;
                }
                info.payout_confirmed = true;
                info.settlement_pending_reason = None;
                info.finalize_sig = (!result.sig.is_empty()).then_some(result.sig);
                if result.winner_lamports > 0 {
                    info.winning_prize = result.winner_lamports;
                }
                // Overwrite the estimate even for zero: a confirmed draw or free game has no payout.
                info.country_fee = result.country_fee;
                info.elo_fee = result.elo_fee;
                info.operating_cost = result.operating_cost_lamports;
                info.fee_breakdown_confirmed = true;
                info.game_ended_at = Some(std::time::Instant::now());
            }
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.finalization_rx = None;
        }
    }
}

fn apply_nonce_resync(mut bridge: ResMut<RollupNetworkBridge>) {
    let rx = match bridge.nonce_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(next_nonce) => {
            bridge.move_nonce = next_nonce;
            bridge.nonce_rx = None;
            info!("[NONCE] Local move_nonce set to {}", next_nonce);
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.nonce_rx = None;
        }
    }
}

fn handle_game_end_pgn_export(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    rollup_manager: Res<EphemeralRollupManager>,
    profile: Res<crate::multiplayer::solana::addon::SolanaProfile>,
    competitive: Res<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut bridge: ResMut<RollupNetworkBridge>,
) {
    for event in game_ended_events.read() {
        if bridge.pgn_rx.is_some() {
            continue;
        }

        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            event.game_id
        };

        // Prefer wallet usernames and ELO; fall back to generic labels when profiles are unavailable.
        let my_name = if profile.username.is_empty() {
            "You".to_string()
        } else {
            profile.username.clone()
        };
        let opponent_name = if competitive.opponent_username.is_empty() {
            "Opponent".to_string()
        } else {
            competitive.opponent_username.clone()
        };
        let my_elo = (competitive.elo_rating > 0).then_some(competitive.elo_rating);
        let opponent_elo = (competitive.opponent_elo > 0).then_some(competitive.opponent_elo);

        let (white_name, black_name, white_elo, black_elo) = if rollup_manager.is_creator {
            (my_name, opponent_name, my_elo, opponent_elo)
        } else {
            (opponent_name, my_name, opponent_elo, my_elo)
        };

        let result_str = match event.winner.as_deref() {
            Some("white") => "1-0",
            Some("black") => "0-1",
            _ => "1/2-1/2",
        }
        .to_string();

        let (tx, rx) = oneshot::channel();
        bridge.pgn_rx = Some(rx);

        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                use crate::game::replay_braid::braid_move_log_to_parsed_pgn_rated;
                use crate::multiplayer::vps_client;

                let moves = match vps_client::fetch_move_log(game_id) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(
                            "[PGN-EXPORT] fetch_move_log failed for game {}: {}",
                            game_id, e
                        );
                        let _ = tx.send(None);
                        return;
                    }
                };

                let pgn = braid_move_log_to_parsed_pgn_rated(
                    &moves,
                    &white_name,
                    &black_name,
                    white_elo,
                    black_elo,
                    &result_str,
                );
                if pgn.is_none() {
                    warn!(
                        "[PGN-EXPORT] Failed to build PGN for game {} ({} moves)",
                        game_id,
                        moves.len()
                    );
                }
                let _ = tx.send(pgn);
            })
            .detach();
    }
}

fn apply_pgn_export_result(
    mut bridge: ResMut<RollupNetworkBridge>,
    mut commands: Commands,
    mut cached_pgn: Option<ResMut<crate::ui::menus::game_over_popup::CachedGamePgn>>,
) {
    let rx = match bridge.pgn_rx.as_mut() {
        Some(rx) => rx,
        None => return,
    };
    match rx.try_recv() {
        Ok(Some(pgn)) => {
            bridge.pgn_rx = None;
            info!(
                "[PGN-EXPORT] Inserting ParsedPgnGameResource ({} moves)",
                pgn.moves.len()
            );

            // Update CachedGamePgn with the authoritative VPS-fetched PGN so that
            // the Review / Analyze / Save PGN buttons use the full Braid move log.
            if let Some(ref mut cached) = cached_pgn {
                let pgn_str = crate::ui::menus::game_over_popup::pgn_to_string(&pgn);
                cached.pgn_string = pgn_str;
                cached.pgn = Some(pgn.clone());
                cached.braid_pgn_ready = true;
                info!("[PGN-EXPORT] CachedGamePgn updated from Braid log");
            }

            commands.insert_resource(ParsedPgnGameResource {
                inner: pgn,
                show_eval_graph: false,
                puzzle_mode: false,
                puzzle_revealed: false,
            });
        }
        Ok(None) => {
            bridge.pgn_rx = None;
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => {
            bridge.pgn_rx = None;
        }
    }
}

fn handle_magic_block_events(
    mut magicblock_events: MessageReader<MagicBlockEvent>,
    mut popup_queue: ResMut<crate::ui::menus::popup::GamePopupQueue>,
) {
    for event in magicblock_events.read() {
        match event {
            MagicBlockEvent::GameDelegated { game_pda } => {
                info!("Magic Block: Game {} delegated to ER", game_pda);
            }
            MagicBlockEvent::GameUndelegated { game_pda } => {
                info!("Magic Block: Game {} undelegated from ER", game_pda);
            }
            MagicBlockEvent::DelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to delegate game {}: {}",
                    game_pda, error
                );
                // Delegation failure is retried by the client and the backend settlement worker.
                popup_queue.push(crate::ui::menus::popup::GamePopup {
                    title: "Ephemeral Rollup sync issue".to_string(),
                    message: "Having trouble syncing this game to the Ephemeral Rollup — retrying automatically.".to_string(),
                    copy_text: None,
                    url: None,
                    url_label: None,
                    lifetime: 8.0,
                    remaining: 8.0,
                    dismissed: false,
                    created_at: std::time::Instant::now(),
                });
            }
            MagicBlockEvent::UndelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to undelegate game {}: {}",
                    game_pda, error
                );
            }
            MagicBlockEvent::TransactionRoutedToEr { signature } => {
                info!("Magic Block: Transaction routed to ER: {}", signature);
            }
        }
    }
}

#[cfg(test)]
mod game_end_ordering_tests {
    use super::*;
    use crate::game::events::GameEndedEvent;
    use crate::multiplayer::rollup::magicblock::DelegationStatus;
    use crate::multiplayer::solana::addon::CompetitiveMatchState;
    use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
    use crate::multiplayer::systems::finalize_game_on_end;
    use crate::multiplayer::types::OnlineNetworkState;
    use bevy::prelude::MinimalPlugins;

    fn build_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        // Register only the ordered systems under test. The full plugin requires
        // unrelated resources that would fail Bevy system validation.
        app.add_message::<GameEndedEvent>();
        app.add_message::<RollupEvent>();
        app.add_message::<MagicBlockEvent>();
        app.insert_resource(RollupNetworkBridge::new());
        app.insert_resource(MagicBlockResolver::default());
        app.init_resource::<RecentTransactions>();

        app.add_systems(
            Update,
            (
                finalize_game_on_end,
                handle_rollup_to_network_events.after(finalize_game_on_end),
                handle_game_end_undelegation,
                retry_pending_finalization.after(handle_rollup_to_network_events),
                poll_game_end_flush,
            ),
        );

        // Queue a move so force_flush emits GameEndBatch and the test exercises the flushing guard.
        let mut mgr = EphemeralRollupManager::new(777, true, "startpos".to_string());
        mgr.add_local_move("g2g4".to_string(), "fen_after_g2g4".to_string());
        app.insert_resource(mgr);

        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        app.insert_resource(SolanaIntegrationState {
            wallet_pubkey: Some(white),
            opponent_pubkey: Some(black),
            ..Default::default()
        });
        app.insert_resource(CompetitiveMatchState {
            wager_lamports: 1_000_000,
            ..Default::default()
        });
        app.insert_resource(OnlineNetworkState::default());

        // Use a delegated Game PDA so the test reaches the undelegation path.
        {
            let mut resolver = app.world_mut().resource_mut::<MagicBlockResolver>();
            resolver.delegation_status = DelegationStatus::Delegated;
            resolver.delegated_game_pda = Some(Pubkey::new_unique());
        }

        app
    }

    #[test]
    fn finalize_never_fires_while_game_end_batch_still_flushing() {
        let mut app = build_test_app();

        app.world_mut().write_message(GameEndedEvent {
            game_id: 777,
            winner: Some("black".to_string()),
            reason: "checkmate".to_string(),
        });

        // The chained systems must set the flushing flag before retry within this update.
        app.update();
        let bridge = app.world().resource::<RollupNetworkBridge>();
        assert!(
            bridge.game_end_moves_flushing,
            "expected the move batch to be flushing after the first frame — \
             test setup problem, not the bug under test, if this fails"
        );
        assert!(
            bridge.finalization_rx.is_none(),
            "REGRESSION: finalize/undelegate fired while the game-end move \
             batch was still flushing — this is the exact race that \
             stranded a real-wager game live on 2026-08-11"
        );

        // The task is still pending on frame 2, so undelegation must remain blocked.
        app.update();
        let bridge = app.world().resource::<RollupNetworkBridge>();
        assert!(
            bridge.finalization_rx.is_none(),
            "REGRESSION: finalize/undelegate fired on a later frame while \
             still flushing"
        );
    }
}

#[cfg(test)]
mod settlement_status_tests {
    use super::*;

    #[test]
    fn confirmed_signature_or_closed_game_proves_settlement() {
        assert_eq!(
            settlement_status(Some(Ok(())), None),
            SettlementStatus::Confirmed
        );
        // finalize_game closes the PDA, so closure is proof even when this
        // client's own request failed (e.g. the settlement worker won).
        assert_eq!(
            settlement_status(None, Some(true)),
            SettlementStatus::Confirmed
        );
        assert_eq!(
            settlement_status(Some(Err("x".into())), Some(true)),
            SettlementStatus::Confirmed
        );
    }

    #[test]
    fn http_success_without_chain_evidence_stays_pending() {
        assert!(matches!(
            settlement_status(None, Some(false)),
            SettlementStatus::Pending(_)
        ));
        // RPC lookup failure is not evidence either way.
        assert!(matches!(
            settlement_status(None, None),
            SettlementStatus::Pending(_)
        ));
        assert!(matches!(
            settlement_status(Some(Err("custom program error".into())), Some(false)),
            SettlementStatus::Pending(_)
        ));
        assert!(matches!(
            FinalizationResult::default().status,
            SettlementStatus::Pending(_)
        ));
    }
}
