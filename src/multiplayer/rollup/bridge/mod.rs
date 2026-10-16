//! Bridges the P2P game and the rollup: relays moves ([`move_relay`]), delegates the
//! game at start ([`delegation`]), settles at the end ([`settlement`]) and exports
//! the PGN ([`pgn_export`]).

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

mod delegation;
mod move_relay;
mod pgn_export;
mod settlement;
use delegation::*;
pub use move_relay::*;
use pgn_export::*;
use settlement::*;

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
    pgn_rx: Option<oneshot::Receiver<Option<xfchess_rules::ParsedPgnGame>>>,
    joiner_delegation_wait_rx: Option<oneshot::Receiver<Result<Pubkey, String>>>,
    joiner_delegation_wait_game_id: Option<u64>,
    game_end_moves_flushing: bool,
    game_end_flush_rx: Option<oneshot::Receiver<()>>,
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
        crate::core::net_task::spawn_detached("bridge", move || {
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
        });
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

        // Game start: delegate the game account to the rollup before the frame's
        // game logic runs.
        app.add_systems(
            Update,
            (
                handle_game_start_delegation,
                poll_delegation_tasks,
                poll_joiner_delegation_wait,
                retry_pending_delegation,
                handle_magic_block_events,
            )
                .chain()
                .in_set(crate::core::schedule::SessionSet::Delegation),
        );
        // Moves and game end, once this frame's moves are applied and the result is
        // known. The order is the settlement protocol: the final batch flush is
        // started (raising `game_end_moves_flushing`) before finalization is queued,
        // and the flush poll runs before the retry, so the retry never undelegates
        // while the last moves are still being submitted.
        app.add_systems(
            Update,
            (
                handle_network_to_rollup_events,
                process_batch_commit_requests,
                handle_rollup_to_network_events,
                handle_game_end_undelegation,
                poll_game_end_flush,
                retry_pending_finalization,
                apply_finalization_result,
                apply_nonce_resync,
                handle_game_end_pgn_export,
                apply_pgn_export_result,
                handle_game_end_causal_cleanup,
            )
                .chain()
                .after(crate::multiplayer::systems::finalize_game_on_end)
                .in_set(crate::core::schedule::SendSet::Settlement),
        );

        info!("RollupNetworkBridgePlugin initialized with Magic Block ER support");
    }
}
