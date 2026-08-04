//! Background task spawning for the XFChess backend.
//!
//! This module handles spawning and managing background tasks
//! such as matchmaking.

use crate::signing::{AnalysisQueue, AppState, SigningConfig, TournamentTrigger};
use crate::tasks::anticheat_worker;
use crate::tasks::archiver;
use crate::tasks::matchmaking;
use crate::tasks::tournament_scheduler::{spawn_prize_distributor, spawn_tournament_scheduler};
use tracing::info;

/// Spawns all background tasks for the application.
///
/// This function spawns:
/// - Matchmaking service (pairs players by ELO)
/// - Tournament scheduler (Braid-based pub/sub triggers)
/// - Prize distributor (auto-pays tournament winners once completed)
/// - Game archiver, anti-cheat workers, durable email queue
///
/// Settlement worker and the anti-cheat re-ingest sweep are spawned
/// separately by `server::run`, once `AppState.anticheat_queue` has actually
/// been populated with this call's return value — see the comment there.
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

    // Auto prize distribution — cranks the permissionless
    // distribute_tournament_prizes instruction once a tournament completes,
    // paying winners without a claim tx.
    spawn_prize_distributor(
        (*state.tournament_store).clone(),
        state.store.pool(),
        Some((
            config.program_id.clone(),
            config.solana_rpc_url.clone(),
            state.vps_authority.clone(),
        )),
    );
    info!("[Tasks] Prize distributor spawned");

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
    // Settlement stays scan-based (chain-derived, already durable, spawned
    // separately by server::run — see the comment there) — see tasks/queue.rs
    // module docs. Prize distribution is also scan-based, via
    // spawn_prize_distributor above.
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
