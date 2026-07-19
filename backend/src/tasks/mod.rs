//! Background tasks for the XFChess backend.
//!
//! This module provides long-running background services:
//! - Matchmaking: ELO-based player pairing
//! - Fee Claimer: Platform fee collection from vault
//! - Tournament Scheduler: auto-start scheduled tournaments

use crate::error::AppError;
use crate::signing::swiss::spawn_orchestrator;
use crate::signing::{
    ws_subscriber::{Cluster, WebSocketSubscriber},
    AppState, OrchestratorEvent, SwissService,
};
use crate::tasks::tournament_scheduler::spawn_tournament_scheduler;
use std::sync::Arc;
use tracing::error;

/// Channel buffer size for orchestrator events.
pub const ORCHESTRATOR_CHANNEL_SIZE: usize = 100;

pub mod anticheat_worker;
pub mod archiver;
pub mod fee_claimer;
pub mod matchmaking;
pub mod queue;
pub mod settlement_worker;
pub mod tournament_scheduler;

pub async fn spawn_background_tasks(mut state: AppState) -> Result<(), AppError> {
    let (orchestrator_tx, _orchestrator_rx) =
        tokio::sync::mpsc::channel::<OrchestratorEvent>(ORCHESTRATOR_CHANNEL_SIZE);
    state.orchestrator_tx = Some(orchestrator_tx);

    let state = Arc::new(state);
    let swiss_service = Arc::new(SwissService::new((*state.tournament_store).clone()));
    let tournament_store = (*state.tournament_store).clone();
    // Initialize WebSocket subscriber for account subscriptions
    let ws_subscriber =
        match WebSocketSubscriber::new(Cluster::Devnet, Some("wss://devnet-eu.magicblock.app"))
            .await
        {
            Ok(sub) => sub,
            Err(e) => {
                error!("[TASKS] Failed to create WebSocket subscriber: {}", e);
                return Err(AppError::WebSocketSubscriptionError(e.to_string()));
            }
        };
    let _ws_subscriber = Arc::new(ws_subscriber);
    let on_chain = Some((
        state.config.program_id.clone(),
        state.config.solana_rpc_url.clone(),
        state.vps_authority.clone(),
        state.tournament_fee_recipient,
    ));
    let _tournament_handle = spawn_tournament_scheduler(
        (*state.tournament_store).clone(),
        Some(state.tournament_gossip.clone()),
        on_chain,
    );
    // Auto prize distribution — pays tournament winners without a claim tx,
    // gated on anti-cheat verdicts for the tournament's games.
    crate::tasks::tournament_scheduler::spawn_prize_distributor(
        (*state.tournament_store).clone(),
        state.store.pool(),
        Some((
            state.config.program_id.clone(),
            state.config.solana_rpc_url.clone(),
            state.vps_authority.clone(),
        )),
    );
    // Auto game settlement — finalizes finished games and pays wager escrows
    // even when the client never calls /game/finalize.
    crate::tasks::settlement_worker::spawn_settlement_worker(state.clone());
    // Re-ingest sweep — retries anti-cheat jobs dropped by a full queue or
    // lost to a restart, rebuilding them from the games table.
    crate::signing::anticheat_enqueue::spawn_reingest_sweep(state.clone());
    // Placeholder for gossip_handle if needed
    let _gossip_handle = tokio::spawn(async move {
        // Placeholder for tournament gossip service
    });
    let _orchestrator_handle = spawn_orchestrator(tournament_store, swiss_service, None);

    Ok(())
}
