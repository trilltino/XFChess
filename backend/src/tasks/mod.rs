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
use tracing::error;

/// Channel buffer size for orchestrator events.
pub const ORCHESTRATOR_CHANNEL_SIZE: usize = 100;

pub mod matchmaking;
pub mod fee_claimer;
pub mod tournament_scheduler;
pub mod archiver;

pub async fn spawn_background_tasks(
    mut state: AppState,
) -> Result<(), AppError> {
    let (orchestrator_tx, _orchestrator_rx) = tokio::sync::mpsc::channel::<OrchestratorEvent>(ORCHESTRATOR_CHANNEL_SIZE);
    state.orchestrator_tx = Some(orchestrator_tx);

    let state = Arc::new(state);
    let swiss_service = Arc::new(SwissService::new((*state.tournament_store).clone()));
    let tournament_store = (*state.tournament_store).clone();
    // Initialize WebSocket subscriber for account subscriptions
    let ws_subscriber = match WebSocketSubscriber::new(
        Cluster::Devnet,
        Some("wss://devnet-eu.magicblock.app"),
    ).await {
        Ok(sub) => sub,
        Err(e) => {
            error!("[TASKS] Failed to create WebSocket subscriber: {}", e);
            return Err(AppError::WebSocketSubscriptionError(e.to_string()));
        }
    };
    let _ws_subscriber = Arc::new(ws_subscriber);
    let _tournament_handle = spawn_tournament_scheduler(
        (*state.tournament_store).clone(),
        Some(state.tournament_gossip.clone()),
    );
    // Placeholder for gossip_handle if needed
    let _gossip_handle = tokio::spawn(async move {
        // Placeholder for tournament gossip service
    });
    let _orchestrator_handle = spawn_orchestrator(
        tournament_store,
        swiss_service,
        None,
    );

    Ok(())
}
