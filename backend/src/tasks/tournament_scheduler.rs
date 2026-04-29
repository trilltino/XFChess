//! Braid-based tournament scheduler — async fill-bracket scheduling.
//!
//! Two cooperating tasks:
//! * [`TournamentScheduler`] consumes events from an [`mpsc`] channel and
//!   decides when to start a tournament.  Two start modes:
//!
//!   - **Async fill** (`scheduled_at = None`): bracket fires as soon as
//!     `max_players` register.  If only `min_players` are present, a
//!     [`FILL_GRACE_SECS`] countdown starts; any additional players extend
//!     nothing — the countdown fires the bracket regardless.  If `max_players`
//!     fills during the grace window the timer is cancelled and the bracket
//!     fires immediately.
//!
//!   - **Scheduled** (`scheduled_at = Some(ts)`): bracket fires at `ts` if
//!     `>= min_players` are registered.  If not enough players arrive within
//!     [`GRACE_SECS`] after `ts`, the tournament is cancelled.
//!
//! * [`spawn_scheduled_start_ticker`] polls every 30 s for tournaments whose
//!   `scheduled_at` has passed.
//!
//! References:
//! * Tokio tasks  — <https://tokio.rs/tokio/tutorial/spawning>
//! * mpsc channel — <https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html>

use crate::signing::storage::tournament::{TournamentStore, TournamentStatus};
use crate::signing::tournament_gossip::TournamentGossipService;
use braid_iroh::protocol::SwissMessage;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// Messages that can trigger tournament actions via Braid pub/sub.
#[derive(Debug, Clone)]
pub enum TournamentTrigger {
    /// Re-evaluate start conditions for a tournament.
    CheckStart { tournament_id: u64 },
    /// A player just joined — may trigger fill-start or grace timer.
    PlayerJoined { tournament_id: u64, player_count: usize },
    /// Admin explicitly requested an immediate start.
    AdminStart { tournament_id: u64 },
    /// Scheduled start time has been reached (emitted by the ticker task).
    ScheduledStart { tournament_id: u64 },
    /// Fill grace timer expired — start if still >= min_players.
    FillGraceExpired { tournament_id: u64 },
}

/// Async-fill tournament scheduler.
pub struct TournamentScheduler {
    store: TournamentStore,
    /// Kept so grace-timer tasks can send `FillGraceExpired` back to us.
    trigger_tx: mpsc::Sender<TournamentTrigger>,
    trigger_rx: mpsc::Receiver<TournamentTrigger>,
    /// Per-tournament grace-timer handles — aborted on max-fill or admin start.
    fill_timers: HashMap<u64, JoinHandle<()>>,
    gossip: Option<Arc<TournamentGossipService>>,
}

impl TournamentScheduler {
    pub fn new(store: TournamentStore) -> (Self, mpsc::Sender<TournamentTrigger>) {
        let (trigger_tx, trigger_rx) = mpsc::channel(256);
        (
            Self {
                store,
                trigger_tx: trigger_tx.clone(),
                trigger_rx,
                fill_timers: HashMap::new(),
                gossip: None,
            },
            trigger_tx,
        )
    }

    pub fn set_gossip(&mut self, gossip: Arc<TournamentGossipService>) {
        self.gossip = Some(gossip);
    }

    pub async fn run(mut self) {
        info!("[tournament-scheduler] Async-fill scheduler started");
        while let Some(trigger) = self.trigger_rx.recv().await {
            match trigger {
                TournamentTrigger::CheckStart { tournament_id } => {
                    self.try_scheduled_start(tournament_id).await;
                }
                TournamentTrigger::PlayerJoined { tournament_id, player_count } => {
                    self.handle_player_joined(tournament_id, player_count).await;
                }
                TournamentTrigger::AdminStart { tournament_id } => {
                    self.abort_fill_timer(tournament_id);
                    self.start_tournament(tournament_id).await;
                }
                TournamentTrigger::ScheduledStart { tournament_id } => {
                    self.try_scheduled_start(tournament_id).await;
                }
                TournamentTrigger::FillGraceExpired { tournament_id } => {
                    self.fill_timers.remove(&tournament_id);
                    self.handle_fill_grace_expired(tournament_id).await;
                }
            }
        }
    }

    /// After this many seconds of waiting at `min_players`, fire the bracket.
    const FILL_GRACE_SECS: u64 = 5 * 60;

    /// After `scheduled_at` passes, cancel the tournament if still below
    /// `min_players` after this many seconds.
    const GRACE_SECS: i64 = 10 * 60;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn abort_fill_timer(&mut self, tournament_id: u64) {
        if let Some(handle) = self.fill_timers.remove(&tournament_id) {
            handle.abort();
            info!(
                "[tournament-scheduler] Cancelled fill-grace timer for tournament {}",
                tournament_id
            );
        }
    }

    // ── event handlers ───────────────────────────────────────────────────────

    async fn handle_player_joined(&mut self, tournament_id: u64, player_count: usize) {
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => return,
        };

        if tournament.status != TournamentStatus::Registration || tournament.scheduled_at.is_some() {
            return;
        }

        let max = tournament.max_players as usize;
        let min = tournament.min_players.unwrap_or(8) as usize;

        if player_count >= max {
            // Bracket is completely full — fire immediately.
            self.abort_fill_timer(tournament_id);
            info!(
                "[tournament-scheduler] Tournament {} filled ({}/{}). Bracket fires now.",
                tournament_id, player_count, max
            );
            self.start_tournament(tournament_id).await;
        } else if player_count >= min && !self.fill_timers.contains_key(&tournament_id) {
            // Hit minimum threshold for the first time — start fill-grace countdown.
            let remaining = max - player_count;
            info!(
                "[tournament-scheduler] Tournament {} hit min players ({}/{}) — {} more could join. \
                 Fill-grace timer: {}s.",
                tournament_id, player_count, max, remaining, Self::FILL_GRACE_SECS
            );
            let tx = self.trigger_tx.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(TournamentScheduler::FILL_GRACE_SECS)).await;
                let _ = tx.send(TournamentTrigger::FillGraceExpired { tournament_id }).await;
            });
            self.fill_timers.insert(tournament_id, handle);
        }
    }

    async fn handle_fill_grace_expired(&self, tournament_id: u64) {
        let tournament = match self.store.get(tournament_id).await {
            Some(t) => t,
            None => return,
        };

        if tournament.status != TournamentStatus::Registration {
            return;
        }

        let min = tournament.min_players.unwrap_or(8) as usize;
        let count = tournament.players.len();

        if count >= min {
            info!(
                "[tournament-scheduler] Fill-grace expired for tournament {} ({}/{} players). Starting.",
                tournament_id, count, tournament.max_players
            );
            self.start_tournament(tournament_id).await;
        } else {
            info!(
                "[tournament-scheduler] Fill-grace expired for tournament {} but only {}/{} players — keeping open.",
                tournament_id, count, min
            );
        }
    }

    async fn try_scheduled_start(&self, tournament_id: u64) {
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

        let min = tournament.min_players.unwrap_or(8) as usize;
        let count = tournament.players.len();

        match tournament.scheduled_at {
            Some(scheduled_at) if now >= scheduled_at => {
                if count >= min {
                    self.start_tournament(tournament_id).await;
                } else if now >= scheduled_at + Self::GRACE_SECS {
                    warn!(
                        "[tournament-scheduler] Tournament {} past grace ({}/{} min). Cancelling.",
                        tournament_id, count, min
                    );
                    let _ = self.store.update_status(tournament_id, TournamentStatus::Cancelled).await;
                } else {
                    info!(
                        "[tournament-scheduler] Tournament {} past scheduled_at, within grace ({}/{} min). Waiting.",
                        tournament_id, count, min
                    );
                }
            }
            _ => {}
        }
    }

    // ── start ─────────────────────────────────────────────────────────────────

    async fn start_tournament(&self, tournament_id: u64) {
        if let Err(e) = self.store.start_tournament(tournament_id).await {
            warn!(
                "[tournament-scheduler] Failed to start tournament {}: {}",
                tournament_id, e
            );
            return;
        }

        let player_count = self
            .store
            .get(tournament_id)
            .await
            .map(|t| t.players.len() as u16)
            .unwrap_or(0);
        let started_at = chrono::Utc::now().timestamp();

        info!(
            "[tournament-scheduler] Tournament {} started with {} players",
            tournament_id, player_count
        );

        // Broadcast BracketFired so connected players know to fetch their match.
        let Some(gossip) = &self.gossip else { return };
        let Some(sender) = gossip.get_topic(tournament_id).await else {
            warn!(
                "[tournament-scheduler] No gossip topic for tournament {} — skipping BracketFired broadcast",
                tournament_id
            );
            return;
        };
        let msg = SwissMessage::BracketFired { tournament_id, player_count, started_at };
        match serde_json::to_vec(&msg) {
            Ok(bytes) => {
                if let Err(e) = sender.broadcast(bytes.into()).await {
                    warn!("[tournament-scheduler] BracketFired broadcast failed for {}: {}", tournament_id, e);
                } else {
                    info!("[tournament-scheduler] BracketFired broadcast sent for tournament {}", tournament_id);
                }
            }
            Err(e) => warn!("[tournament-scheduler] BracketFired serialize error: {}", e),
        }
    }
}

/// Spawn the scheduler as a background task.
///
/// Pass `gossip` so the scheduler can broadcast [`SwissMessage::BracketFired`]
/// when a tournament auto-starts.
pub fn spawn_tournament_scheduler(
    store: TournamentStore,
    gossip: Option<Arc<TournamentGossipService>>,
) -> mpsc::Sender<TournamentTrigger> {
    let (mut scheduler, trigger_tx) = TournamentScheduler::new(store.clone());
    if let Some(g) = gossip {
        scheduler.set_gossip(g);
    }
    tokio::spawn(scheduler.run());
    spawn_scheduled_start_ticker(store, trigger_tx.clone());
    trigger_tx
}

/// Tick interval for the scheduled-start scanner.
const SCHEDULED_START_TICK: Duration = Duration::from_secs(30);

/// Spawns a background task that scans all tournaments every
/// [`SCHEDULED_START_TICK`] and emits `ScheduledStart` for any tournament
/// whose `scheduled_at` has passed while still in `Registration`.
pub fn spawn_scheduled_start_ticker(
    store: TournamentStore,
    trigger_tx: mpsc::Sender<TournamentTrigger>,
) {
    tokio::spawn(async move {
        info!(
            "[tournament-scheduler] Scheduled-start ticker: {}s interval",
            SCHEDULED_START_TICK.as_secs()
        );
        let mut ticker = tokio::time::interval(SCHEDULED_START_TICK);
        ticker.tick().await; // skip immediate first tick to avoid startup race

        loop {
            ticker.tick().await;
            let now = chrono::Utc::now().timestamp();
            for t in store.list().await {
                if t.status != TournamentStatus::Registration {
                    continue;
                }
                let Some(scheduled_at) = t.scheduled_at else { continue };
                if now < scheduled_at {
                    continue;
                }
                let trigger = TournamentTrigger::ScheduledStart { tournament_id: t.tournament_id };
                if let Err(e) = trigger_tx.send(trigger).await {
                    warn!(
                        "[tournament-scheduler] Channel closed, dropping ScheduledStart for {}: {}",
                        t.tournament_id, e
                    );
                    return;
                }
            }
        }
    });
}
