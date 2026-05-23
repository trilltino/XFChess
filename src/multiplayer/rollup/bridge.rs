// Rollup Network Bridge for MagicBlock ER
use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::game::events::{GameEndedEvent, GameStartedEvent};
use crate::multiplayer::{
    MagicBlockEvent, MagicBlockResolver,
    calculate_batch_hash, NetworkMessage,
    EphemeralRollupManager, GameStateStatus, RollupEvent,
    BraidNetworkState,
    NetworkEvent,
};
use crate::multiplayer::rollup::magicblock::DelegationStatus;
use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
use crate::solana::instructions::PROGRAM_ID as SOLANA_PROGRAM_ID;
use crate::ui::menus::game_over_popup::GameOverPayoutInfo;

/// Result sent back from the async finalization task to the Bevy world.
#[derive(Debug, Default)]
struct FinalizationResult {
    sig: String,
    winner_lamports: u64,
    country_fee: u64,
}

/// Maximum seconds to wait for the Game PDA to return to devnet after undelegation.
const MAX_UNDELEGATE_WAIT_SECS: u64 = 60;

/// Stores the last few on-chain move transaction signatures so the UI can display them.
#[derive(Resource, Default, Clone)]
pub struct RecentTransactions {
    /// Ring buffer of (move_uci, tx_signature) tuples, newest last.
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
    /// Hashes of batches we proposed ourselves — used to suppress gossip self-echoes.
    sent_batch_hashes: std::collections::HashSet<String>,
    /// PDA stored when delegation failed because wallet info wasn't ready yet.
    pending_delegation_pda: Option<Pubkey>,
    /// game_id matching pending_delegation_pda.
    pending_game_id: Option<u64>,
    /// Channel receiving delegation result from async task.
    delegation_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    /// Monotonically increasing nonce for record_move replay protection.
    /// Starts at 1 because the program requires nonce == move_log.nonce + 1 (on-chain starts at 0).
    move_nonce: u64,
    /// Finalization deferred because opponent pubkey was not yet available at game end.
    /// Retried each frame up to MAX_FINALIZATION_WAIT_FRAMES.
    pending_finalization: Option<PendingFinalization>,
    /// Channel receiving the finalization result (sig + payout amounts) from the async task.
    finalization_rx: Option<oneshot::Receiver<FinalizationResult>>,
    /// Channel receiving the resynced move nonce from the async RPC fetch.
    nonce_rx: Option<oneshot::Receiver<u64>>,
}

/// Maximum frames to wait for opponent pubkey before giving up on deferred finalization.
/// At 60 fps this is ~10 seconds.
const MAX_FINALIZATION_WAIT_FRAMES: u32 = 600;

/// Captures the data needed to finalize a game when `opponent_pubkey` was not yet
/// available in `SolanaIntegrationState` at the moment `GameEndedEvent` fired.
#[derive(Debug)]
struct PendingFinalization {
    game_id: u64,
    winner: Option<String>,
    is_delegated: bool,
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
}

pub struct RollupNetworkBridgePlugin;

impl Plugin for RollupNetworkBridgePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RollupNetworkBridge::new());

        let mut resolver = MagicBlockResolver::default();
        resolver.set_solana_rpc(Arc::new(RpcClient::new(
            "https://api.devnet.solana.com",
        )));
        app.insert_resource(resolver);

        app.init_resource::<RecentTransactions>();
        app.add_message::<MagicBlockEvent>();

        // Core network bridge systems
        app.add_systems(Update, handle_rollup_to_network_events);
        app.add_systems(Update, handle_network_to_rollup_events);
        app.add_systems(Update, process_batch_commit_requests);

        // Magic Block ER delegation systems
        app.add_systems(Update, handle_game_start_delegation);
        app.add_systems(Update, retry_pending_delegation);
        app.add_systems(Update, handle_game_end_undelegation);
        app.add_systems(Update, handle_magic_block_events);

        app.add_systems(Update, poll_delegation_tasks);
        app.add_systems(Update, retry_pending_finalization);

        // Post-finalization: apply payout result to game-over popup resource.
        app.add_systems(Update, apply_finalization_result);
        // Nonce resync: apply on-chain nonce once the async fetch completes.
        app.add_systems(Update, apply_nonce_resync);

        info!("RollupNetworkBridgePlugin initialized with Magic Block ER support");
    }
}

fn send_network_msg(state: &BraidNetworkState, msg: NetworkMessage) {
    if let Some(tx) = &state.message_sender {
        if let Err(e) = tx.send(msg) {
            warn!("Failed to send NetworkMessage: {}", e);
        }
    }
}

fn handle_rollup_to_network_events(
    mut rollup_events: MessageReader<RollupEvent>,
    network_state: Res<BraidNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    rollup_manager: Res<EphemeralRollupManager>,
) {
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
            // Final game-end batch: skip BatchPropose/Accept and submit directly to VPS.
            // This ensures moves are recorded even if the peer disconnects after checkmate.
            RollupEvent::GameEndBatch {
                game_id,
                moves,
                next_fens,
            } => {
                let gid = *game_id;
                let base_nonce = bridge.move_nonce;
                bridge.move_nonce += moves.len() as u64;
                let moves_owned = moves.clone();
                let fens_owned = next_fens.clone();
                info!(
                    "[VPS] Game-end direct submit: {} moves for game {}",
                    moves_owned.len(), gid
                );
                bevy::tasks::IoTaskPool::get()
                    .spawn(async move {
                        use crate::multiplayer::vps_client;
                        for (i, (mv, fen)) in moves_owned.iter().zip(fens_owned.iter()).enumerate() {
                            match vps_client::record_move(gid, mv, fen, base_nonce + i as u64) {
                                Ok(sig) => info!("[VPS] Move {} recorded game {} sig {}", mv, gid, sig),
                                Err(e) => error!("[VPS] record_move failed {} game {}: {}", mv, gid, e),
                            }
                        }
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
    network_state: Res<BraidNetworkState>,
    mut rollup_events: MessageWriter<RollupEvent>,
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut bridge: ResMut<RollupNetworkBridge>,
) {
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

                info!("Peer batch validated for game {} — peer will submit via record_move", game_id);
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
                if let Some((moves, next_fens)) = bridge.pending_batches.remove(batch_hash.as_str()) {
                    let gid = *game_id;
                    let base_nonce = bridge.move_nonce;
                    bridge.move_nonce += moves.len() as u64;
                    bevy::tasks::IoTaskPool::get()
                        .spawn(async move {
                            use crate::multiplayer::vps_client;
                            for (i, (mv, fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
                                match vps_client::record_move(gid, mv, fen, base_nonce + i as u64) {
                                    Ok(sig) => info!("[VPS] Move {} recorded game {} sig {}", mv, gid, sig),
                                    Err(e) => error!("[VPS] record_move failed {} game {}: {}", mv, gid, e),
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
                if *game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = new_fen.clone();
                    rollup_manager.committed_turn = *new_turn;
                    rollup_manager.status = GameStateStatus::Synced;
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
                if *game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = committed_fen.clone();
                    rollup_manager.committed_turn = *committed_turn;
                    rollup_manager.status = GameStateStatus::Synced;
                    info!(
                        "Resynced game {} from peer, turn {}",
                        game_id, committed_turn
                    );
                }
            }

            NetworkMessage::Move { .. } => {
                // Individual move broadcasts are handled by the game sync layer.
                // Do NOT add to the local pending_batch — that must only contain
                // moves made by the local player.
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
) {
    if bridge.awaiting_commit_confirmation {
        return;
    }
    if rollup_manager.status != GameStateStatus::Pending || !rollup_manager.should_flush() {
        return;
    }

    if let Some((moves, next_fens)) = rollup_manager.prepare_batch_for_commit() {
        let base_nonce = bridge.move_nonce;
        bridge.move_nonce += moves.len() as u64;
        submit_moves_via_vps(
            rollup_manager.game_id,
            &moves,
            &next_fens,
            base_nonce,
            &mut magicblock_events,
            &mut recent_txs,
        );
        bridge.awaiting_commit_confirmation = true;
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

/// Submit moves via the VPS signing service (zero wallet popups).
fn submit_moves_via_vps(
    game_id: u64,
    moves: &[String],
    next_fens: &[String],
    base_nonce: u64,
    magicblock_events: &mut MessageWriter<MagicBlockEvent>,
    recent_txs: &mut RecentTransactions,
) {
    use crate::multiplayer::vps_client;

    for (i, (move_str, next_fen)) in moves.iter().zip(next_fens.iter()).enumerate() {
        match vps_client::record_move(game_id, move_str, next_fen, base_nonce + i as u64) {
            Ok(sig) => {
                info!("[VPS] Move {} recorded for game {}: {}", move_str, game_id, sig);
                recent_txs.push(move_str.clone(), sig.clone());
                magicblock_events.write(MagicBlockEvent::TransactionRoutedToEr { signature: sig });
            }
            Err(e) => {
                error!("[VPS] record_move failed for {} game {}: {}", move_str, game_id, e);
                return;
            }
        }
    }
}

/// Handles game start events to delegate the game PDA to the Ephemeral Rollup
///
/// This system listens for GameStartedEvent and spawns an async task to perform
/// the delegation off the main thread, preventing Bevy from freezing.
fn handle_game_start_delegation(
    mut game_started_events: MessageReader<GameStartedEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    rollup_manager: Res<EphemeralRollupManager>,
) {
    for event in game_started_events.read() {
        // Use the Solana on-chain game_id, not the P2P gossip game_id.
        // event.game_id is the Braid/Iroh session ID; rollup_manager.game_id
        // is set from the actual on-chain game account after create/join.
        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            warn!(
                "[DELEGATION] rollup_manager.game_id is 0 at GameStarted (p2p id {}); deferring",
                event.game_id
            );
            continue;
        };

        // Only the game creator (white player) delegates.
        // If both players delegate simultaneously the second TX fails with
        // AccountOwnedByWrongProgram because the PDA owner changed after the first delegation.
        if !rollup_manager.is_creator {
            info!("[DELEGATION] Game {} — joiner does not delegate; skipping", game_id);
            continue;
        }

        info!("[DELEGATION] Game {} started - spawning ER delegation task", game_id);

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

        let (tx, rx) = oneshot::channel();
        bridge.delegation_rx = Some(rx);

        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                let result =
                    spawn_delegation_task(game_pda, game_id, wallet_pubkey, rpc_client).await;
                let _ = tx.send(result);
            })
            .detach();

        // Item 5: fetch on-chain nonce so we never start with a stale local nonce.
        let (nonce_tx, nonce_rx) = oneshot::channel::<u64>();
        bridge.nonce_rx = Some(nonce_rx);
        bevy::tasks::IoTaskPool::get()
            .spawn(async move {
                use crate::multiplayer::vps_client;
                match vps_client::vps_fetch_move_nonce(game_id) {
                    Ok(next_nonce) => {
                        info!("[NONCE] Resynced nonce for game {} → {}", game_id, next_nonce);
                        let _ = nonce_tx.send(next_nonce);
                    }
                    Err(e) => {
                        warn!("[NONCE] Failed to fetch nonce for game {}: {} — keeping local nonce", game_id, e);
                    }
                }
            })
            .detach();
    }
}

/// Async delegation task that runs on IoTaskPool (off main thread).
///
/// Builds the delegation instruction and signs via Tauri (wallet popup).
/// The delegation ix marks wallet_pubkey as is_signer:true, so only the wallet can sign.
async fn spawn_delegation_task(
    game_pda: Pubkey,
    game_id: u64,
    wallet_pubkey: Pubkey,
    rpc_client: Arc<RpcClient>,
) -> Result<Pubkey, String> {
    use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
    use crate::multiplayer::solana::tauri_signer;

    info!(
        "[DELEGATION-TASK] Starting delegation for game {} (PDA: {})",
        game_id, game_pda
    );

    let mut resolver = crate::multiplayer::rollup::magicblock::MagicBlockResolver::default();
    resolver.set_solana_rpc(rpc_client.clone());
    resolver.set_game_id(game_id);

    let ix = resolver
        .create_delegation_instruction(game_pda, wallet_pubkey)
        .map_err(|e| format!("build delegation ix: {}", e))?;

    // The delegation instruction marks wallet_pubkey as is_signer:true, so the
    // wallet must sign — not the VPS session key. Route through Tauri (Phantom popup).
    info!("[DELEGATION-TASK] Sending delegation TX via Tauri wallet for game {}", game_id);

    match tauri_signer::sign_and_send_via_tauri(DEVNET_RPC_URL, wallet_pubkey, &[ix], &[]) {
        Ok(sig) => {
            info!("[DELEGATION-TASK] SUCCESS for game {} sig: {}", game_id, sig);
            Ok(game_pda)
        }
        Err(e) => {
            error!("[DELEGATION-TASK] FAILED for game {}: {}", game_id, e);
            Err(e)
        }
    }
}

/// Polls the delegation async task and emits events on completion.
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
            }
            Ok(Err(e)) => {
                error!("Delegation failed: {}", e);
                // Store the PDA for retry
                if let Some(pda) = bridge.pending_delegation_pda {
                    magicblock_events.write(MagicBlockEvent::DelegationFailed {
                        game_pda: pda,
                        error: e,
                    });
                }
                bridge.delegation_rx = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Task still running, nothing to do
            }
            Err(_) => {
                error!("Delegation task dropped");
                bridge.delegation_rx = None;
            }
        }
    }
}

/// Retries a previously-deferred ER delegation once the wallet info is available.
fn retry_pending_delegation(
    mut bridge: ResMut<RollupNetworkBridge>,
    magicblock_resolver: Res<MagicBlockResolver>,
    solana_state: Option<Res<SolanaIntegrationState>>,
    magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    if bridge.delegation_rx.is_some() {
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

    bridge.pending_delegation_pda = None;
    bridge.pending_game_id = None;

    let (tx, rx) = oneshot::channel();
    bridge.delegation_rx = Some(rx);

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result =
                spawn_delegation_task(game_pda, game_id, wallet_pubkey, rpc_client).await;
            let _ = tx.send(result);
        })
        .detach();

    info!("Retry delegation spawned for game {} PDA {}", game_id, game_pda);

    let _ = magicblock_events; // suppress unused warning
}

/// Handles game end events to undelegate the game PDA from the Ephemeral Rollup
/// and finalize the game result on devnet — all signed by the VPS session key.
///
/// Flow (spawned async so Bevy never blocks):
///   1. POST /game/undelegate → ER commits state to devnet
///   2. sleep 3 s (let commit land)
///   3. POST /game/finalize  → devnet: status=Finished, wager payout, ELO update
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
        // Use the Solana on-chain game_id (rollup_manager), not the P2P event ID.
        let game_id = if rollup_manager.game_id != 0 {
            rollup_manager.game_id
        } else {
            event.game_id
        };

        info!("[FINALIZE] Game {} ended (winner={:?} reason={}) — preparing on-chain finalization",
            game_id, event.winner, event.reason);

        // Derive and log the move_log PDA so the user can look up moves on Solscan.
        let program_id: solana_sdk::pubkey::Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let move_log_pda = solana_sdk::pubkey::Pubkey::find_program_address(
            &[b"move_log", &game_id.to_le_bytes()],
            &program_id,
        ).0;
        info!("[FINALIZE] move_log PDA: {}", move_log_pda);
        info!("[FINALIZE] Solscan: https://solscan.io/account/{}?cluster=devnet", move_log_pda);

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
                warn!("[FINALIZE] No wallet state — cannot finalize game {}", game_id);
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
            warn!("[FINALIZE] Opponent pubkey unavailable for game {} — deferring finalization", game_id);
            let local_pk = solana_state.as_ref().and_then(|s| s.wallet_pubkey).unwrap_or_default();
            let wager = competitive.as_ref().map(|c| c.stake_amount).unwrap_or(0);
            bridge.pending_finalization = Some(PendingFinalization {
                game_id,
                winner: event.winner.clone(),
                is_delegated,
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
                    if let Err(e) = vps_client::vps_submit_free_rated_result(game_id, win.as_deref(), &w, &b) {
                        error!("[FREE_RATED] ELO update failed for game {}: {e}", game_id);
                    } else {
                        info!("[FREE_RATED] ELO updated for game {}", game_id);
                    }
                })
                .detach();
            continue;
        }

        let wager = competitive.as_ref().map(|c| c.stake_amount).unwrap_or(0);
        let (fin_tx, fin_rx) = oneshot::channel::<FinalizationResult>();
        bridge.finalization_rx = Some(fin_rx);
        spawn_finalization_task(game_id, winner, white_pk, black_pk, wager, fin_tx);
    }
}

/// Spawns the async undelegate + finalize flow off the Bevy main thread.
/// Item 2: Polls the Game PDA owner on devnet instead of a fixed sleep.
/// Item 1: Sends the finalization result back to Bevy via `result_tx`.
fn spawn_finalization_task(
    game_id: u64,
    winner: Option<String>,
    white_pk: Pubkey,
    black_pk: Pubkey,
    wager_lamports: u64,
    result_tx: oneshot::Sender<FinalizationResult>,
) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            use crate::multiplayer::vps_client;
            use solana_client::rpc_client::RpcClient;
            use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
            use crate::solana::instructions::PROGRAM_ID as SOLANA_PROGRAM_ID;

            // Brief pause before undelegation so the final move batch lands first.
            std::thread::sleep(std::time::Duration::from_secs(2));

            match vps_client::vps_undelegate_game(game_id) {
                Ok(sig) => info!("[UNDELEGATE] ER committed for game {} sig {}", game_id, sig),
                Err(e) => error!("[UNDELEGATE] Failed for game {}: {e} — continuing to finalize", game_id),
            }

            // Item 2: Poll devnet until game PDA owner returns to the program (not ER).
            let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
            let game_pda = Pubkey::find_program_address(
                &[b"game", &game_id.to_le_bytes()], &program_id,
            ).0;
            let rpc = RpcClient::new(DEVNET_RPC_URL.to_string());
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(MAX_UNDELEGATE_WAIT_SECS);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                match rpc.get_account(&game_pda) {
                    Ok(acc) if acc.owner == program_id => {
                        info!("[UNDELEGATE] Game {} PDA returned to devnet — proceeding to finalize", game_id);
                        break;
                    }
                    Ok(_) => {} // still owned by ER
                    Err(_) => {} // transient RPC error — keep polling
                }
                if std::time::Instant::now() >= deadline {
                    warn!("[UNDELEGATE] Game {} PDA did not return after {}s — finalizing anyway",
                          game_id, MAX_UNDELEGATE_WAIT_SECS);
                    break;
                }
            }

            let w_str = white_pk.to_string();
            let b_str = black_pk.to_string();
            let win_ref = winner.as_deref();

            match vps_client::vps_finalize_game(game_id, win_ref, &w_str, &b_str, wager_lamports) {
                Ok(result) => {
                    info!("[FINALIZE] Game {} finalized on-chain sig {} prize {} lam",
                          game_id, result.sig, result.winner_lamports);
                    let _ = result_tx.send(FinalizationResult {
                        sig: result.sig,
                        winner_lamports: result.winner_lamports,
                        country_fee: result.country_fee,
                    });
                }
                Err(e) => {
                    error!("[FINALIZE] Game {} finalization failed: {e}", game_id);
                    // Send a zero-prize result so the UI still shows the popup correctly.
                    let _ = result_tx.send(FinalizationResult::default());
                }
            }
        })
        .detach();
}

/// Retries a deferred game finalization each frame until `opponent_pubkey` becomes
/// available in `SolanaIntegrationState` (set by `handle_session_info_from_network`)
/// or `MAX_FINALIZATION_WAIT_FRAMES` elapses.
fn retry_pending_finalization(
    mut bridge: ResMut<RollupNetworkBridge>,
    solana_state: Option<Res<SolanaIntegrationState>>,
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
        warn!("[FINALIZE] Resolved pubkeys still default for game {} — skipping", pending.game_id);
        return;
    }

    info!(
        "[FINALIZE] Opponent pubkey arrived after {} frames for game {} — finalizing",
        pending.frames_waited, pending.game_id
    );
    let (fin_tx, fin_rx) = oneshot::channel::<FinalizationResult>();
    bridge.finalization_rx = Some(fin_rx);
    spawn_finalization_task(pending.game_id, pending.winner, white_pk, black_pk, pending.wager_lamports, fin_tx);
}

/// Item 1: Reads the finalization result channel and updates GameOverPayoutInfo.
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
                info.payout_confirmed = true;
                info.finalize_sig = Some(result.sig);
                if result.winner_lamports > 0 {
                    info.winning_prize = result.winner_lamports;
                }
                if result.country_fee > 0 {
                    info.country_fee = result.country_fee;
                }
                info.game_ended_at = Some(std::time::Instant::now());
            }
        }
        Err(oneshot::error::TryRecvError::Empty) => {}
        Err(_) => { bridge.finalization_rx = None; }
    }
}

/// Item 5: Applies the resynced on-chain nonce once the async fetch completes.
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
        Err(_) => { bridge.nonce_rx = None; }
    }
}

/// Handles Magic Block events for logging and error handling
fn handle_magic_block_events(mut magicblock_events: MessageReader<MagicBlockEvent>) {
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

