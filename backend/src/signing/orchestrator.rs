//! Drives every tournament's games to a result, whatever the format.
//!
//! The settlement worker reports each settled tournament game as
//! [`OrchestratorEvent::GameEnded`], draws included. The orchestrator applies it by
//! the event's own rules ([`crate::signing::elimination`] or
//! [`crate::signing::swiss`]) and then mirrors the event on-chain through that
//! format's reconcile pass. Reconciling also runs on a timer, so a restart, a
//! dropped event or a failed transaction is repaired later; the orchestrator keeps
//! no state of its own.

use std::sync::Arc;
use std::time::Duration;

use solana_sdk::signature::Keypair;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::signing::elimination::{self, EliminationError, EliminationService, GameApplied};
use crate::signing::storage::tournament::{
    FormatState, GameOutcome, TournamentRecord, TournamentStore,
};
use crate::signing::swiss::{self, SwissService, SwissServiceError};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// A tournament game settled on-chain. `white`/`black` are the on-chain Game's
    /// players, checked against the match the game id belongs to.
    GameEnded {
        tournament_id: u64,
        game_id: u64,
        white: String,
        black: String,
        outcome: GameOutcome,
    },
    /// Off-chain state changed (admin result, forfeit, new round); mirror it now.
    Reconcile { tournament_id: u64 },
}

/// Where and as whom tournament results are written on-chain.
#[derive(Clone)]
pub struct ChainConfig {
    pub program_id: String,
    pub rpc_url: String,
    /// Must be each tournament's on-chain `authority`.
    pub authority: Arc<Keypair>,
}

pub struct TournamentOrchestrator {
    store: TournamentStore,
    swiss: Arc<SwissService>,
    elimination: EliminationService,
    event_rx: mpsc::Receiver<OrchestratorEvent>,
    on_chain: Option<ChainConfig>,
}

impl TournamentOrchestrator {
    pub fn new(
        store: TournamentStore,
        swiss: Arc<SwissService>,
        on_chain: Option<ChainConfig>,
    ) -> (Self, mpsc::Sender<OrchestratorEvent>) {
        let (tx, rx) = mpsc::channel(256);
        let orchestrator = Self {
            elimination: EliminationService::new(store.clone()),
            store,
            swiss,
            event_rx: rx,
            on_chain,
        };
        (orchestrator, tx)
    }

    pub async fn run(mut self) {
        info!("[orchestrator] tournament orchestrator started");
        let mut tick = tokio::time::interval(RECONCILE_INTERVAL);
        loop {
            tokio::select! {
                event = self.event_rx.recv() => match event {
                    Some(OrchestratorEvent::GameEnded { tournament_id, game_id, white, black, outcome }) => {
                        self.game_ended(tournament_id, game_id, &white, &black, outcome).await;
                        self.reconcile(tournament_id).await;
                    }
                    Some(OrchestratorEvent::Reconcile { tournament_id }) => {
                        self.reconcile(tournament_id).await;
                    }
                    None => break,
                },
                _ = tick.tick() => self.reconcile_all().await,
            }
        }
        info!("[orchestrator] tournament orchestrator stopped");
    }

    async fn game_ended(
        &self,
        tournament_id: u64,
        game_id: u64,
        white: &str,
        black: &str,
        outcome: GameOutcome,
    ) {
        let Some(t) = self.store.get(tournament_id).await else {
            warn!("[orchestrator] game {game_id}: tournament {tournament_id} not found");
            return;
        };
        match t.state {
            FormatState::Elimination(_) => {
                match self
                    .elimination
                    .record_game(tournament_id, game_id, white, black, outcome)
                    .await
                {
                    Ok(GameApplied::Decided(d)) => info!(
                        "[orchestrator] tournament {tournament_id} match {}: {} beat {}",
                        d.match_index, d.winner, d.loser
                    ),
                    Ok(GameApplied::Replay {
                        match_index,
                        game_id: replay,
                    }) => info!(
                        "[orchestrator] tournament {tournament_id} match {match_index}: game \
                         {game_id} drawn, replay {replay:?} with colours reversed"
                    ),
                    // A settlement retry for a game already applied.
                    Err(EliminationError::UnknownGame(_)) => {}
                    Err(e) => error!(
                        "[orchestrator] tournament {tournament_id} game {game_id} not applied: {e}"
                    ),
                }
            }
            FormatState::Swiss(_) => {
                match self
                    .swiss
                    .record_game(tournament_id, game_id, white, black, outcome)
                    .await
                {
                    Ok((round, board)) => info!(
                        "[orchestrator] tournament {tournament_id} round {round} board {board}: \
                         {outcome:?}"
                    ),
                    Err(SwissServiceError::ResultAlreadyRecorded) => {}
                    Err(e) => error!(
                        "[orchestrator] tournament {tournament_id} game {game_id} not applied: {e}"
                    ),
                }
            }
        }
    }

    async fn reconcile(&self, tournament_id: u64) {
        if self.on_chain.is_none() {
            return;
        }
        if let Some(t) = self.store.get(tournament_id).await {
            self.reconcile_record(&t).await;
        }
    }

    async fn reconcile_record(&self, t: &TournamentRecord) {
        let Some(cfg) = &self.on_chain else {
            return;
        };
        match &t.state {
            FormatState::Elimination(_) => {
                elimination::chain::reconcile(&self.elimination, cfg, t).await
            }
            FormatState::Swiss(_) => swiss::chain::reconcile(&self.store, cfg, t).await,
        }
    }

    async fn reconcile_all(&self) {
        if self.on_chain.is_none() {
            return;
        }
        for t in self.store.chain_sync_pending().await {
            self.reconcile_record(&t).await;
        }
    }
}

pub fn spawn_orchestrator(
    store: TournamentStore,
    swiss: Arc<SwissService>,
    on_chain: Option<ChainConfig>,
) -> mpsc::Sender<OrchestratorEvent> {
    let (orchestrator, tx) = TournamentOrchestrator::new(store, swiss, on_chain);
    tokio::spawn(orchestrator.run());
    tx
}
