//! Background task spawning for the XFChess backend.
//!
//! This module handles spawning and managing background tasks
//! such as matchmaking and fee claiming.

use crate::signing::{AnalysisQueue, AppState, SigningConfig, TournamentTrigger};
use crate::tasks::anticheat_worker;
use crate::tasks::archiver;
use crate::tasks::fee_claimer;
use crate::tasks::matchmaking;
use crate::tasks::tournament_scheduler::spawn_tournament_scheduler;
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
/// `(tournament_trigger, anticheat_queue)` — callers set these on `AppState`.
pub fn spawn_background_tasks(
    state: AppState,
    config: SigningConfig,
) -> (tokio::sync::mpsc::Sender<TournamentTrigger>, AnalysisQueue) {
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

    // Spawn tournament scheduler (with gossip so it can broadcast BracketFired)
    let tournament_store = (*state.tournament_store).clone();
    let gossip = Some(state.tournament_gossip.clone());
    let on_chain = Some((
        config.program_id.clone(),
        config.solana_rpc_url.clone(),
        state.vps_authority.clone(),
        state.tournament_fee_recipient,
    ));
    let trigger_tx = spawn_tournament_scheduler(tournament_store, gossip, on_chain);
    info!("[Tasks] Tournament scheduler spawned with async-fill and gossip broadcast");

    // Spawn game archiver
    let pool = state.store.pool();
    tokio::spawn(async move {
        archiver::run_archiver_service(pool).await;
    });
    info!("[Tasks] Game archiver service spawned");

    // Spawn anti-cheat Stockfish analysis workers
    let ac_pool = state.store.pool();
    let ac_queue = anticheat_worker::spawn_anticheat_workers(ac_pool);
    info!("[Tasks] Anti-cheat analysis workers spawned");

    // Spawn the durable job queue worker (email delivery with retries + DLQ).
    // Settlement/prizes stay scan-based (chain-derived, already durable) — see
    // tasks/queue.rs module docs.
    crate::tasks::queue::QueueWorker::new()
        .register(
            "email.send",
            crate::signing::routes::mailer::handle_email_job,
        )
        .spawn(state.store.pool());
    info!("[Tasks] Durable job-queue worker spawned (email.send)");

    info!("[Tasks] All background tasks spawned successfully");
    (trigger_tx, ac_queue)
}

#[cfg(test)]
mod tests {
    // AppState and SigningConfig construction tests would require
    // full dependency injection of all 14+ fields. Integration tests
    // for task spawning belong in tests/ or with mocked state.
}
