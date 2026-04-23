//! Background tasks for the XFChess backend.
//!
//! This module provides long-running background services:
//! - Matchmaking: ELO-based player pairing
//! - Fee Claimer: Platform fee collection from vault
//! - Tournament Scheduler: auto-start scheduled tournaments

use crate::error::AppError;
use crate::signing::{AppState, SwissService, OrchestratorEvent, ws_subscriber::{WebSocketSubscriber, Cluster}};
use crate::tasks::tournament_scheduler::spawn_tournament_scheduler;
use crate::signing::swiss::spawn_orchestrator;
use std::sync::Arc;

pub mod matchmaking;
pub mod fee_claimer;
pub mod tournament_scheduler;

pub async fn spawn_background_tasks(
    mut state: AppState,
) -> Result<(), AppError> {
    let (orchestrator_tx, orchestrator_rx) = tokio::sync::mpsc::channel::<OrchestratorEvent>(100);
    state.orchestrator_tx = Some(orchestrator_tx);

    let state = Arc::new(state);
    let swiss_service = Arc::new(SwissService::new((*state.tournament_store).clone()));
    let tournament_store = (*state.tournament_store).clone();
    // Initialize WebSocket subscriber for account subscriptions
    let ws_subscriber = WebSocketSubscriber::new(
        Cluster::Devnet,
        Some("wss://devnet-eu.magicblock.app"),
        100,
    ).await.unwrap();
    let ws_subscriber = Arc::new(ws_subscriber);
    let tournament_handle = spawn_tournament_scheduler(
        (*state.tournament_store).clone(),
    );
    // Placeholder for gossip_handle if needed
    let gossip_handle = tokio::spawn(async move {
        // Placeholder for tournament gossip service
    });
    let orchestrator_handle = spawn_orchestrator(
        tournament_store,
        swiss_service,
        None,
    );

    Ok(())
}
