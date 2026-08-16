//! Bridges Bevy game events to the MagicBlock Ephemeral Rollup lifecycle.
//!
//! On [`GameStartedEvent`], the creator (only) delegates the game PDA to the
//! ER off the main thread ([`spawn_delegation_task`]/[`retry_pending_delegation`]),
//! polled to completion by [`poll_delegation_tasks`]. On [`GameEndedEvent`], the
//! reverse happens: the VPS is asked to undelegate ([`vps_client::vps_undelegate_game`]),
//! this system waits for the game PDA to return to devnet, then fires the
//! finalize/settlement flow. Free (non-wagered) games skip delegation entirely
//! and update ELO directly. See `crates/CLAUDE.md` for how this fits with the
//! Braid/P2P relay layer.
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

/// Result sent back from the async finalization task to the Bevy world.
#[derive(Debug, Default)]
struct FinalizationResult {
    sig: String,
    winner_lamports: u64,
    country_fee: u64,
    /// Real backend-advanced operating cost reimbursed to treasury_vault
    /// from the pot (0 for free games — see `vps::game::FinalizeResult`).
    operating_cost_lamports: u64,
    /// Flat ELO-linking fee split between both players (0 for free games).
    elo_fee: u64,
}

/// Maximum seconds to wait for the Game PDA to return to devnet after undelegation.
const MAX_UNDELEGATE_WAIT_SECS: u64 = 60;

/// Maximum seconds the joiner waits to observe the creator's delegation
/// landing on-chain before giving up (see `wait_for_delegation`).
const MAX_JOINER_DELEGATION_WAIT_SECS: u64 = 60;

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
    /// PDA of the game currently being (re)delegated, or awaiting retry after
    /// a failure (precondition-not-ready, signing, or broadcast). Set right
    /// before every delegation attempt is spawned; cleared only on confirmed
    /// success (see `poll_delegation_tasks`).
    pending_delegation_pda: Option<Pubkey>,
    /// game_id matching pending_delegation_pda.
    pending_game_id: Option<u64>,
    /// Channel receiving delegation result from async task.
    delegation_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    /// Seconds remaining before `retry_pending_delegation` will attempt
    /// again after a genuine signing/broadcast failure. Without this, a
    /// real RPC error (as opposed to the "wallet not ready yet" case, which
    /// naturally paces itself on wallet-connect) would retry every single
    /// frame — a retry storm against the RPC endpoint.
    delegation_retry_cooldown: f32,
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
    /// Channel receiving the assembled PGN game after game-end export.
    pgn_rx: Option<oneshot::Receiver<Option<nimzovich_engine::ParsedPgnGame>>>,
    /// Channel receiving the joiner-side delegation-observed result (see
    /// `wait_for_delegation`). Deliberately separate from `delegation_rx`:
    /// that channel feeds `retry_pending_delegation`, which resubmits a
    /// `delegate_game` transaction on failure — the joiner must never do
    /// that (it isn't `game.fee_payer`; the on-chain check would just
    /// reject it), so its wait loop needs its own channel that nothing else
    /// retries against.
    joiner_delegation_wait_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    /// game_id currently being waited on by `joiner_delegation_wait_rx`, so a
    /// repeated `GameStartedEvent` for the same game doesn't spawn a second
    /// waiter task.
    joiner_delegation_wait_game_id: Option<u64>,
    /// True while the final game-end move batch (from `RollupEvent::GameEndBatch`,
    /// see `handle_rollup_to_network_events`) is still being submitted.
    /// `handle_game_end_undelegation` must not undelegate while this is true —
    /// undelegating while the game-ending move (e.g. the checkmate move) is
    /// still in flight races the ER's own commit, which the ER rejects with
    /// `InvalidWritableAccount` ("illegally used as writable"), permanently
    /// stranding that game's result (and any wager) since the move never
    /// lands and the game can never reach `Finished`. Reproduced live
    /// 2026-08-10 on a real-wager game. Set when the batch task is spawned,
    /// cleared by `poll_game_end_flush` once it completes.
    game_end_moves_flushing: bool,
    /// Signals completion of the in-flight game-end move batch (see
    /// `game_end_moves_flushing`). `Ok(())` regardless of whether individual
    /// moves succeeded — this only tracks "attempted", since the game state
    /// itself (not this flag) is the source of truth for whether the game
    /// actually finished.
    game_end_flush_rx: Option<oneshot::Receiver<()>>,
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

    /// True while an on-chain finalization (undelegate + `finalize_game`,
    /// which pays out any wager) is queued or in flight — either still
    /// waiting on `retry_pending_finalization`'s preconditions, or already
    /// spawned and awaiting its result, or the game-end move batch that must
    /// land before undelegation can even be attempted.
    pub fn has_pending_finalization(&self) -> bool {
        self.pending_finalization.is_some()
            || self.finalization_rx.is_some()
            || self.game_end_moves_flushing
            || self.game_end_flush_rx.is_some()
    }

    /// Resets everything except an in-flight finalization. Used when leaving
    /// `InGame` (see `reset_multiplayer_session_state`): a full `Default`
    /// reset there was silently discarding `pending_finalization`/
    /// `finalization_rx` whenever the player dismissed the game-over prompt
    /// before `retry_pending_finalization` had gotten around to firing —
    /// permanently stranding the wager payout with no error or log, since
    /// the async task that would have logged `[UNDELEGATE]`/`[FINALIZED]`
    /// was simply never spawned. Reproduced live 2026-08-16.
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

        // Core network bridge systems
        //
        // `handle_rollup_to_network_events` must run after
        // `systems::finalize_game_on_end` (which decides whether there's a
        // move batch to flush and, if so, emits `RollupEvent::GameEndBatch`)
        // and `retry_pending_finalization` must run after
        // `handle_rollup_to_network_events` (which is what actually sets
        // `game_end_moves_flushing = true` for that batch). Without this
        // explicit chain, Bevy doesn't guarantee these three run in this
        // order within the same tick, so `retry_pending_finalization` could
        // check the flag before it's been set — reproduced live 2026-08-11:
        // deferring by "one pass through pending_finalization" alone wasn't
        // enough, because `retry_pending_finalization` could still run
        // later in the *same* frame as the system that sets the flag, which
        // isn't a real frame boundary. Explicit ordering is the actual fix;
        // the one-frame defer in `handle_game_end_undelegation` is now
        // redundant with this but harmless to leave in place.
        app.add_systems(
            Update,
            handle_rollup_to_network_events.after(crate::multiplayer::systems::finalize_game_on_end),
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
        // Drop this game's causal-chain tracking state so it doesn't accumulate
        // forever across a long client session (tournament play, many spectated
        // games, etc.).
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

/// Resolves both players' wallet pubkeys — mirrors the identical inline
/// logic previously duplicated at the finalization call sites.
/// `is_creator ↔ white; joiner ↔ black`.
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

/// Which wallet made the move at 1-based ply `nonce` — White moves on odd
/// plies, Black on even (matches the on-chain `apply_recorded_move`'s
/// `game.turn % 2 == 1 -> white` check exactly, since `nonce` and
/// `game.turn` track the same ply count).
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
            // Final game-end batch: skip BatchPropose/Accept and submit directly to VPS.
            // This ensures moves are recorded even if the peer disconnects after checkmate.
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
                // Gate undelegation (see `handle_game_end_undelegation`) until
                // this batch — which may include the game-ending move itself
                // — has actually been attempted. Set before spawning, cleared
                // by `poll_game_end_flush` once `flush_tx` fires.
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

            // ── Braid reconnection recovery ───────────────────────────────────
            //
            // A reconnecting peer sends BraidResyncRequest with the version hash
            // of the last move it applied.  We look up our local Braid move log
            // and replay every update that came after that version.
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

                        // Find the position of since_version in the log and return everything after.
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

            // A new peer joined the game gossip topic and broadcast their full
            // current game state.  If we are a spectator or have missed moves,
            // apply the snapshot to catch up.
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
        bridge.move_nonce += moves.len() as u64;
        submit_moves_via_vps(
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
    white_pk: Pubkey,
    black_pk: Pubkey,
    magicblock_events: &mut MessageWriter<MagicBlockEvent>,
    recent_txs: &mut RecentTransactions,
    fallback_er_endpoint: &str,
) {
    use crate::multiplayer::rollup::magicblock::er_explorer_url_for;
    use crate::multiplayer::vps_client;

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
            }
            Err(e) => {
                error!(
                    "[VPS] record_move failed for {} game {}: {}",
                    move_str, game_id, e
                );
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
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
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
            // The joiner still needs to learn *when* delegation lands —
            // `can_move_color` (game/systems/input.rs) blocks all moves
            // until `magicblock_resolver.is_delegated()` is true, and that
            // flag only ever gets set locally by whoever ran the delegation
            // task (`poll_delegation_tasks`). Without this, the joiner's own
            // resolver never transitions out of `Undelegated` and they can
            // never move for the entire game. So the joiner instead polls
            // the game PDA's owner until it becomes the MagicBlock
            // Delegation Program, mirroring the reverse wait already done
            // for undelegation below.
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

        // Sign with the session key that *this specific game* actually used
        // (`rollup_manager.used_global_session`, set once at create/join
        // time) — NOT the wallet's live `global_session_active` flag, which
        // can flip independently of which flow created this game and would
        // pick the wrong key (`FeePayerMismatch` on-chain). When it used the
        // global session, that key co-signs both slots the delegation
        // instruction needs, no wallet popup at all. Otherwise the VPS
        // delegates on our behalf (it holds the per-game session key) —
        // still no wallet popup, just a different signer.
        let global_session_keypair_bytes = solana_state
            .as_ref()
            .filter(|_| rollup_manager.used_global_session)
            .and_then(|s| s.global_session_keypair.as_ref())
            .map(|kp| kp.to_bytes().to_vec());
        let _ = wallet_pubkey; // only used above to gate readiness

        // Set unconditionally (not just in the deferred-precondition branches
        // above) so a genuine signing/broadcast failure — not just "wallet
        // wasn't ready yet" — also has a PDA/game_id for `poll_delegation_tasks`
        // to fire `DelegationFailed` with, and for `retry_pending_delegation`
        // to retry against. Cleared only on confirmed success, in
        // `poll_delegation_tasks`.
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
        let (nonce_tx, nonce_rx) = oneshot::channel::<u64>();
        bridge.nonce_rx = Some(nonce_rx);
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
}

/// Async delegation task that runs on IoTaskPool (off main thread).
///
/// When `global_session_keypair_bytes` is `Some`, signs and submits directly
/// with that session key — zero wallet popup. Otherwise asks the VPS to
/// delegate on our behalf, since it holds the per-game session key that
/// `game.fee_payer` requires — no wallet popup either way.
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
        // `game.fee_payer` is the global session key for games created via
        // `global_create_game`/`global_join_game` — it can satisfy both the
        // `payer` (bookkeeping rent) and `fee_payer` (authority check) slots
        // itself, entirely locally, no Tauri round-trip.
        let session_kp = solana_sdk::signature::Keypair::try_from(kp_bytes.as_slice())
            .map_err(|e| format!("session keypair: {e}"))?;
        let ix = resolver
            .create_delegation_instruction(game_pda, session_kp.pubkey(), session_kp.pubkey())
            .map_err(|e| format!("build delegation ix: {}", e))?;
        // Uses the shared fast submit+poll path (skip_preflight, 150ms poll,
        // 2s deadline) instead of the SDK-default `send_and_confirm_transaction`,
        // which runs preflight simulation — the only write path in this
        // codebase that used to, adding a needless extra RPC round trip.
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

    // Fallback: per-game session flow. `fee_payer` must equal `game.fee_payer`
    // (the per-game session key the *backend* holds, not the client) — so
    // the client can't sign that slot itself. Ask the VPS to delegate on our
    // behalf instead (it already holds that key), same trust model as the
    // existing `vps_undelegate_game`/`vps_finalize_game` calls — no wallet
    // popup here either.
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

/// Joiner-side counterpart to `spawn_delegation_task`. The joiner never
/// submits a `delegate_game` transaction itself (see the non-creator branch
/// of `handle_game_start_delegation`), so it has no local signal that
/// delegation actually completed. Poll the game PDA's owner until it
/// becomes the MagicBlock Delegation Program — the same on-chain signal
/// `spawn_finalization_task` already polls for in reverse (owner returning
/// to the xfchess program after undelegation).
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

/// Polls the joiner's delegation-observation task and applies the result
/// locally once it lands. This is the only place a joiner's own
/// `MagicBlockResolver` ever transitions to `Delegated` — without it,
/// `can_move_color`'s ER-not-delegated gate (`game/systems/input.rs`) blocks
/// the joiner from moving for the entire game.
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

/// Clears `game_end_moves_flushing` once the game-end move batch (spawned in
/// `handle_rollup_to_network_events`'s `GameEndBatch` arm) finishes attempting
/// every move — the signal `handle_game_end_undelegation`/
/// `retry_pending_finalization` wait on before undelegating, see
/// `game_end_moves_flushing`'s doc comment for why.
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
                // Only cleared on confirmed success — a failure leaves these
                // set so `retry_pending_delegation` picks the same game back
                // up next frame instead of losing track of it.
                bridge.pending_delegation_pda = None;
                bridge.pending_game_id = None;
            }
            Ok(Err(e)) => {
                error!("Delegation failed: {}", e);
                // `pending_delegation_pda` is now always set before a
                // delegation task is spawned (see `handle_game_start_delegation`
                // / `retry_pending_delegation`), so this fires for a genuine
                // signing/broadcast failure too, not just the deferred
                // wallet-not-ready case.
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

/// Retries a previously-deferred ER delegation once the wallet info is available.
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

    // Backs off after a genuine signing/broadcast failure (see
    // `poll_delegation_tasks`) — without this, a real RPC error would retry
    // every frame instead of just the "wallet not ready yet" case, which
    // naturally paces itself on wallet-connect.
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

    // `pending_delegation_pda`/`pending_game_id` are deliberately NOT cleared
    // here — only `poll_delegation_tasks` clears them, and only on confirmed
    // success. If this attempt also fails, they need to still be set so the
    // next retry (after another cooldown) can find this same game again.

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

/// Drops [`CausalChainState`] entries for a finished game — otherwise
/// `last_seq`/`head_version`/`roster` grow forever across a client session
/// that plays or spectates many games (e.g. a tournament run).
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
        // Same single-authoritative-side reasoning as `finalize_game_on_end`'s
        // gate (systems.rs): both players' clients detect game end locally and
        // deterministically, so without this check both processes independently
        // race to undelegate/finalize the same delegated PDA. Restrict the
        // on-chain settlement pipeline to the host; `settlement_worker.rs` on
        // the backend is the safety net if the host's client dies mid-flow.
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

        // Deliberately never fire `spawn_finalization_task` on this same
        // frame, even when `white_pk`/`black_pk` are already known. This
        // system and `finalize_game_on_end` (which decides whether there's a
        // move batch to flush, in `systems.rs`) both read the same raw
        // `GameEndedEvent` with no ordering constraint between them — Bevy
        // does not guarantee `finalize_game_on_end` (and the downstream
        // `handle_rollup_to_network_events` that actually sets
        // `game_end_moves_flushing`) has run before this system does on the
        // same tick. Checking the flag here inline was tried and failed live
        // 2026-08-11: the flag still read `false` on the first frame even
        // though a flush was about to start, so undelegate fired immediately
        // anyway and lost the exact same race. Always deferring by at least
        // one frame guarantees every other same-tick system — including the
        // one that sets the flag — has already run by the time
        // `retry_pending_finalization` actually checks it.
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

            // Brief pause before requesting undelegation: the ER processes the
            // last recorded move(s) asynchronously relative to this task, and
            // undelegating too early can race the final move commit. There is
            // no queryable "last move landed" signal to poll on instead (the
            // move record and the undelegation request go through different
            // paths), so this stays a fixed sleep rather than a poll loop —
            // unlike the PDA-ownership wait below, which does poll actual
            // on-chain state. Not reduced without live devnet verification
            // that a shorter pause still reliably avoids the race.
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
                    info!(
                        "[FINALIZED] Game {} settled on-chain, payout {} lamports to winner, sig {}",
                        game_id, result.winner_lamports, result.sig
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

    // See `game_end_moves_flushing`'s doc comment — must not undelegate while
    // the game-end move batch is still being submitted. Shares the same
    // frame budget as the opponent-pubkey wait above rather than a separate
    // counter; either reason blocking this long is equally worth giving up on.
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
                // Overwrite unconditionally (not `if > 0`) — a real 0 (draw,
                // free game) is a confirmed value, not "still unknown", and
                // the pre-finalize estimate set in fetch_game_payout_info
                // must not outlive a successful response.
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
        Err(_) => {
            bridge.nonce_rx = None;
        }
    }
}

/// Fetch the Braid move log from the VPS at game end, convert to PGN, and
/// put the result on a oneshot channel so `apply_pgn_export_result` can insert
/// `ParsedPgnGameResource` from the Bevy main thread.
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

        // Real wallet usernames/ELO when known (Solana PVP), falling back to
        // generic labels only if the profile/match resources haven't been
        // populated yet (e.g. a free game with no on-chain profile fetch).
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

/// Poll the PGN export channel and, when ready, insert `ParsedPgnGameResource`
/// so the replay UI can be opened immediately.
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

/// Handles Magic Block events for logging and error handling
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
                // Previously logged only, with nothing telling the player.
                // Backed now by both `retry_pending_delegation` (client-side
                // retry, with a cooldown) and the backend settlement worker's
                // redelegate-retry for a still-stuck game — this is
                // informational, not the fix itself.
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
    //! Headless regression test for the bug fixed 2026-08-11: a real-wager
    //! game's checkmate move raced `handle_game_end_undelegation`'s
    //! undelegate call, because nothing guaranteed the game-end move batch
    //! (`finalize_game_on_end` -> `RollupEvent::GameEndBatch` ->
    //! `handle_rollup_to_network_events` setting `game_end_moves_flushing`)
    //! had actually started — let alone finished — before finalize fired.
    //! Two prior attempts at this fix both failed live because they assumed
    //! Bevy orders systems by declaration or by an artificial one-frame
    //! defer; neither is a real guarantee. The actual fix is the explicit
    //! `.after()` chain in `RollupNetworkBridgePlugin::build`. This test
    //! exercises the real plugin (no mocked scheduling) and asserts the
    //! exact invariant that broke live: `finalization_rx` (set only right
    //! before the undelegate/finalize network task is spawned) must never
    //! become `Some` while `game_end_moves_flushing` is `true`. No window,
    //! GPU, wallet, or live backend involved — `bevy::tasks::IoTaskPool`
    //! tasks run on a background thread and are never awaited here; only
    //! the synchronous portion of each system (which is what orders the
    //! flag write against the finalize check) is under test.
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

        // Deliberately NOT adding the full `RollupNetworkBridgePlugin`: most
        // of its other systems (delegation polling, PGN export, popup
        // queueing, causal-chain cleanup...) need resources/message types
        // that belong to entirely different plugins in the real app and are
        // irrelevant to the ordering bug under test. Bevy 0.19 panics the
        // whole `update()` if *any* scheduled system's parameters fail
        // validation, so pulling in the whole plugin here would fail before
        // ever reaching the systems actually being tested. Register only
        // the four systems whose relative order matters, with the exact
        // same `.after()` chain `RollupNetworkBridgePlugin::build` uses.
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

        // A pending move so `finalize_game_on_end`'s `force_flush()` returns
        // `Some(..)` and actually emits `RollupEvent::GameEndBatch` — an
        // empty batch would never set `game_end_moves_flushing` at all,
        // which would trivially (and misleadingly) pass this test.
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

        // Delegated + a real PDA — otherwise `handle_game_end_undelegation`
        // takes the free-rated (never-delegated) early-return path instead
        // of the one under test.
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

        // Frame 1: the event is processed. With the explicit `.after()`
        // chain, `finalize_game_on_end` -> `handle_rollup_to_network_events`
        // (sets the flag) -> `retry_pending_finalization` (must see it set)
        // all resolve within this single `update()` call.
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

        // Frame 2: still flushing (the spawned IoTaskPool task hasn't been
        // awaited — nothing here drives it to completion), so this must
        // still hold even once `handle_game_end_undelegation` has had a
        // second chance to populate `pending_finalization`.
        app.update();
        let bridge = app.world().resource::<RollupNetworkBridge>();
        assert!(
            bridge.finalization_rx.is_none(),
            "REGRESSION: finalize/undelegate fired on a later frame while \
             still flushing"
        );
    }
}
