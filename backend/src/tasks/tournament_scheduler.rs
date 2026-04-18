//! Braid-based tournament scheduler - pub/sub triggers instead of polling

use crate::signing::storage::tournament::{TournamentStore, TournamentStatus, TournamentFormat};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Messages that can trigger tournament actions via Braid pub/sub
#[derive(Debug, Clone)]
pub enum TournamentTrigger {
    /// Check if a tournament should start
    CheckStart { tournament_id: u64 },
    /// Player joined - may trigger auto-start
    PlayerJoined { tournament_id: u64, player_count: usize },
    /// Admin explicitly requested start
    AdminStart { tournament_id: u64 },
    /// Scheduled start time reached
    ScheduledStart { tournament_id: u64 },
}

/// Internal scheduler implementation - not exported

/// Braid subscriber for tournament triggers
pub struct TournamentScheduler {
    store: TournamentStore,
    trigger_rx: mpsc::Receiver<TournamentTrigger>,
}

impl TournamentScheduler {
    pub fn new(store: TournamentStore) -> (Self, mpsc::Sender<TournamentTrigger>) {
        let (trigger_tx, trigger_rx) = mpsc::channel(100);
        (Self { store, trigger_rx }, trigger_tx)
    }

    pub async fn run(mut self) {
        info!("[tournament-scheduler] Starting Braid-based scheduler");
        while let Some(trigger) = self.trigger_rx.recv().await {
            match trigger {
                TournamentTrigger::CheckStart { tournament_id } => {
                    self.try_auto_start(tournament_id).await;
                }
                TournamentTrigger::PlayerJoined { tournament_id, player_count } => {
                    self.handle_player_joined(tournament_id, player_count).await;
                }
                TournamentTrigger::AdminStart { tournament_id } => {
                    self.force_start(tournament_id).await;
                }
                TournamentTrigger::ScheduledStart { tournament_id } => {
                    self.try_auto_start(tournament_id).await;
                }
            }
        }
    }

    async fn try_auto_start(&self, tournament_id: u64) {
        let now = chrono::Utc::now().timestamp();
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => {
                warn!("[tournament-scheduler] Tournament {} not found", tournament_id);
                return;
            }
        };

        if tournament.status != TournamentStatus::Registration {
            return;
        }

        let should_start = match tournament.scheduled_at {
            Some(scheduled_at) => {
                now >= scheduled_at && tournament.players.len() >= tournament.min_players.unwrap_or(8) as usize
            }
            None => tournament.players.len() >= tournament.max_players as usize,
        };

        if should_start {
            self.start_tournament(tournament_id).await;
        } else if tournament.scheduled_at.map(|s| now >= s).unwrap_or(false) {
            let _ = self.store.update_status(tournament_id, TournamentStatus::Cancelled).await;
        }
    }

    async fn handle_player_joined(&self, tournament_id: u64, player_count: usize) {
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => return,
        };

        if tournament.scheduled_at.is_none() && player_count >= tournament.max_players as usize {
            self.try_auto_start(tournament_id).await;
        }
    }

    async fn force_start(&self, tournament_id: u64) {
        self.start_tournament(tournament_id).await;
    }

    async fn start_tournament(&self, tournament_id: u64) {
        if let Err(e) = self.store.start_tournament(tournament_id).await {
            warn!("[tournament-scheduler] Failed to start tournament {}: {}", tournament_id, e);
            return;
        }
        info!("[tournament-scheduler] Started tournament {}", tournament_id);
    }
}

/// Spawn the scheduler as a background task
pub fn spawn_tournament_scheduler(store: TournamentStore) -> mpsc::Sender<TournamentTrigger> {
    let (scheduler, trigger_tx) = TournamentScheduler::new(store);
    tokio::spawn(scheduler.run());
    trigger_tx
}
