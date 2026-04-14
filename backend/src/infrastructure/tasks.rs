//! Background task spawning for the XFChess backend.
//!
//! This module handles spawning and managing background tasks
//! such as matchmaking and fee claiming.

use crate::signing::{AppState, SigningConfig};
use crate::tasks::matchmaking;
use crate::tasks::fee_claimer;
use tracing::info;

/// Spawns all background tasks for the application.
///
/// This function spawns:
/// - Matchmaking service (pairs players by ELO)
/// - Fee claimer service (checks and claims platform fees)
///
/// # Arguments
/// * `state` - The shared application state
/// * `config` - The signing configuration
pub fn spawn_background_tasks(state: AppState, config: SigningConfig) {
    // Spawn matchmaking service
    let matchmaking_state = state.matchmaking.clone();
    tokio::spawn(async move {
        matchmaking::run_matchmaking_service(matchmaking_state).await;
    });

    // Spawn fee claimer service
    let rpc_url = config.solana_rpc_url.clone();
    let program_id_str = config.program_id.clone();
    tokio::spawn(async move {
        fee_claimer::run_fee_claimer_service(rpc_url, program_id_str).await;
    });

    info!("[Tasks] All background tasks spawned successfully");
}
