use crate::signing::{AnalysisQueue, AppState, SigningConfig, TournamentTrigger};
use crate::tasks::anticheat_worker;
use crate::tasks::archiver;
use crate::tasks::matchmaking;
use crate::tasks::tournament_scheduler::{spawn_prize_distributor, spawn_tournament_scheduler};
use tracing::info;

pub fn spawn_background_tasks(
    state: AppState,
    config: SigningConfig,
) -> (tokio::sync::mpsc::Sender<TournamentTrigger>, AnalysisQueue) {
    let matchmaking_state = state.matchmaking.clone();
    tokio::spawn(async move {
        matchmaking::run_matchmaking_service(matchmaking_state).await;
    });

    let tournament_store = (*state.tournament_store).clone();
    let braid_hub = Some(state.braid_hub.clone());
    let on_chain = Some((
        config.program_id.clone(),
        config.solana_rpc_url.clone(),
        state.vps_authority.clone(),
        state.tournament_fee_recipient,
    ));
    let trigger_tx = spawn_tournament_scheduler(tournament_store, braid_hub, on_chain);
    info!("[Tasks] Tournament scheduler spawned with async-fill and Braid publication");

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

    let pool = state.store.pool();
    tokio::spawn(async move {
        archiver::run_archiver_service(pool).await;
    });
    info!("[Tasks] Game archiver service spawned");

    let ac_pool = state.store.pool();
    let ac_queue = anticheat_worker::spawn_anticheat_workers(ac_pool);
    info!("[Tasks] Anti-cheat analysis workers spawned");

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
