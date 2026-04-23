//! Braid-based tournament scheduler - pub/sub triggers instead of polling.
//!
//! Two cooperating tasks:
//! * [`TournamentScheduler`] consumes events from an [`mpsc`] channel
//!   (`PlayerJoined`, `AdminStart`, explicit `ScheduledStart`, etc.) and
//!   decides whether to start a tournament.
//! * [`spawn_scheduled_start_ticker`] is a low-frequency time source: once
//!   per 30s it scans all tournaments and emits a `ScheduledStart` event for
//!   any tournament whose `scheduled_at` has arrived but is still in
//!   `Registration`. This is what makes "create today, starts Friday 8pm"
//!   actually fire without any player action.
//!
//! References:
//! * Tokio tasks  — <https://tokio.rs/tokio/tutorial/spawning>
//! * mpsc channel — <https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html>

use crate::signing::storage::tournament::{TournamentStore, TournamentStatus, TournamentFormat};
use std::sync::Arc;
use std::time::Duration;
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

    /// Grace period after `scheduled_at` before cancelling for insufficient
    /// players. 10 minutes matches common tournament practice.
    const GRACE_SECS: i64 = 10 * 60;

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

        let min_players = tournament.min_players.unwrap_or(8) as usize;
        let has_enough = tournament.players.len() >= min_players;

        let should_start = match tournament.scheduled_at {
            Some(scheduled_at) => now >= scheduled_at && has_enough,
            None => tournament.players.len() >= tournament.max_players as usize,
        };

        if should_start {
            self.start_tournament(tournament_id).await;
        } else if let Some(scheduled_at) = tournament.scheduled_at {
            // Past scheduled time but not enough players.
            if now >= scheduled_at && !has_enough {
                let past_grace = now >= scheduled_at + Self::GRACE_SECS;
                if past_grace {
                    warn!(
                        "[tournament-scheduler] Tournament {} past grace ({} players < {} min). Cancelling.",
                        tournament_id, tournament.players.len(), min_players
                    );
                    let _ = self.store.update_status(tournament_id, TournamentStatus::Cancelled).await;
                } else {
                    info!(
                        "[tournament-scheduler] Tournament {} past scheduled_at but within grace ({} < {} min). Waiting.",
                        tournament_id, tournament.players.len(), min_players
                    );
                }
            }
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

/// Spawn the scheduler as a background task.
///
/// Also spawns a low-frequency ticker (every 30 seconds) that emits
/// `ScheduledStart` triggers for tournaments whose `scheduled_at` has
/// arrived while still in `Registration`. Without this, scheduled
/// tournaments would only auto-start when a player joins or an admin pokes
/// the scheduler manually.
pub fn spawn_tournament_scheduler(store: TournamentStore) -> mpsc::Sender<TournamentTrigger> {
    let (scheduler, trigger_tx) = TournamentScheduler::new(store.clone());
    tokio::spawn(scheduler.run());
    spawn_scheduled_start_ticker(store, trigger_tx.clone());
    trigger_tx
}

/// Tick interval for the scheduled-start scanner.
///
/// 30s is a good compromise: short enough for timely auto-starts,
/// long enough that the DB scan is negligible even with thousands of rows.
const SCHEDULED_START_TICK: Duration = Duration::from_secs(30);

/// Spawns a background task that scans all tournaments every
/// [`SCHEDULED_START_TICK`] and emits a `ScheduledStart` trigger for each
/// tournament whose `scheduled_at` has passed while it is still in
/// `Registration`.
///
/// The scheduler task dedupes transitions: sending `ScheduledStart` for a
/// tournament already `Active` is a no-op inside `try_auto_start`.
pub fn spawn_scheduled_start_ticker(
    store: TournamentStore,
    trigger_tx: mpsc::Sender<TournamentTrigger>,
) {
    tokio::spawn(async move {
        info!(
            "[tournament-scheduler] Scheduled-start ticker running every {}s",
            SCHEDULED_START_TICK.as_secs()
        );
        let mut ticker = tokio::time::interval(SCHEDULED_START_TICK);
        // Skip the immediate first tick fired by Interval::new so we do not
        // race the app's startup sequence.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let now = chrono::Utc::now().timestamp();
            let tournaments = store.list().await;

            for t in tournaments {
                if t.status != TournamentStatus::Registration {
                    continue;
                }
                let Some(scheduled_at) = t.scheduled_at else { continue };
                if now < scheduled_at {
                    continue;
                }

                let trigger = TournamentTrigger::ScheduledStart {
                    tournament_id: t.tournament_id,
                };
                if let Err(e) = trigger_tx.send(trigger).await {
                    warn!(
                        "[tournament-scheduler] Dropping scheduled-start trigger for {}: {}",
                        t.tournament_id, e
                    );
                    // channel closed; stop the ticker
                    return;
                }
            }
        }
    });
}
