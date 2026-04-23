//! Background task spawning for the XFChess backend.
//!
//! This module handles spawning and managing background tasks
//! such as matchmaking and fee claiming.

use crate::signing::{AppState, SigningConfig, TournamentTrigger};
use crate::tasks::matchmaking;
use crate::tasks::fee_claimer;
use crate::tasks::tournament_scheduler::spawn_tournament_scheduler;
use std::sync::Arc;
use tracing::info;

/// Spawns all background tasks for the application.
///
/// This function spawns:
/// - Matchmaking service (pairs players by ELO)
/// - Fee claimer service (checks and claims platform fees)
/// - Tournament scheduler (Braid-based pub/sub triggers)
///
/// # Arguments
/// * `state` - The shared application state
/// * `config` - The signing configuration
///
/// # Returns
/// Tournament trigger sender for route handlers
pub fn spawn_background_tasks(state: AppState, config: SigningConfig) -> tokio::sync::mpsc::Sender<TournamentTrigger> {
    // Spawn matchmaking service
    let matchmaking_state = state.matchmaking.clone();
    tokio::spawn(async move {
        matchmaking::run_matchmaking_service(matchmaking_state).await;
    });

    // Spawn fee claimer service
    let rpc_url = config.solana_rpc_url.clone();
    let program_id_str = config.program_id.clone();
    let feepayer = state.feepayer.clone();
    tokio::spawn(async move {
        fee_claimer::run_fee_claimer_service(rpc_url, program_id_str, feepayer).await;
    });

    // Spawn tournament scheduler
    let tournament_store = (*state.tournament_store).clone();
    let trigger_tx = spawn_tournament_scheduler(tournament_store);
    info!("[Tasks] Tournament scheduler spawned with Braid pub/sub");

    info!("[Tasks] All background tasks spawned successfully");
    trigger_tx
}

#[cfg(test)]
mod tests {
    // AppState and SigningConfig construction tests would require
    // full dependency injection of all 14+ fields. Integration tests
    // for task spawning belong in tests/ or with mocked state.
}
