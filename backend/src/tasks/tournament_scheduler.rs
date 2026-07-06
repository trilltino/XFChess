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

use crate::signing::storage::tournament::{TournamentStatus, TournamentStore};
use crate::signing::tournament_gossip::TournamentGossipService;
use braid_iroh::SwissMessage;
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Channel buffer size for tournament trigger events.
pub const TOURNAMENT_TRIGGER_CHANNEL_SIZE: usize = 256;

/// Messages that can trigger tournament actions via Braid pub/sub.
#[derive(Debug, Clone)]
pub enum TournamentTrigger {
    /// Re-evaluate start conditions for a tournament.
    CheckStart { tournament_id: u64 },
    /// A player just joined — may trigger fill-start or grace timer.
    PlayerJoined {
        tournament_id: u64,
        player_count: usize,
    },
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
    /// On-chain config — present when the backend has a VPS authority key.
    on_chain: Option<OnChainConfig>,
}

struct OnChainConfig {
    program_id: String,
    rpc_url: String,
    vps_authority: Arc<Keypair>,
    /// Operator treasury — receives swept entry fees at start_tournament.
    host_treasury: solana_sdk::pubkey::Pubkey,
}

impl TournamentScheduler {
    pub fn new(store: TournamentStore) -> (Self, mpsc::Sender<TournamentTrigger>) {
        let (trigger_tx, trigger_rx) = mpsc::channel(TOURNAMENT_TRIGGER_CHANNEL_SIZE);
        (
            Self {
                store,
                trigger_tx: trigger_tx.clone(),
                trigger_rx,
                fill_timers: HashMap::new(),
                gossip: None,
                on_chain: None,
            },
            trigger_tx,
        )
    }

    pub fn set_gossip(&mut self, gossip: Arc<TournamentGossipService>) {
        self.gossip = Some(gossip);
    }

    pub fn set_on_chain(
        &mut self,
        program_id: String,
        rpc_url: String,
        vps_authority: Arc<Keypair>,
        host_treasury: solana_sdk::pubkey::Pubkey,
    ) {
        self.on_chain = Some(OnChainConfig {
            program_id,
            rpc_url,
            vps_authority,
            host_treasury,
        });
    }

    pub async fn run(mut self) {
        info!("[tournament-scheduler] Async-fill scheduler started");
        while let Some(trigger) = self.trigger_rx.recv().await {
            match trigger {
                TournamentTrigger::CheckStart { tournament_id } => {
                    self.try_scheduled_start(tournament_id).await;
                }
                TournamentTrigger::PlayerJoined {
                    tournament_id,
                    player_count,
                } => {
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

        if tournament.status != TournamentStatus::Registration || tournament.scheduled_at.is_some()
        {
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
                let _ = tx
                    .send(TournamentTrigger::FillGraceExpired { tournament_id })
                    .await;
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
                warn!(
                    "[tournament-scheduler] Tournament {} not found",
                    tournament_id
                );
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
                    let _ = self
                        .store
                        .update_status(tournament_id, TournamentStatus::Cancelled)
                        .await;
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
        // ── On-chain: start_tournament + initialize_match × N ────────────────
        if let Some(cfg) = &self.on_chain {
            let max_players = self
                .store
                .get(tournament_id)
                .await
                .map(|t| t.max_players)
                .unwrap_or(0);

            let program_id_str = cfg.program_id.clone();
            let rpc_url = cfg.rpc_url.clone();
            let authority = cfg.vps_authority.clone();
            let host_treasury = cfg.host_treasury;
            let total_matches = max_players.saturating_sub(1) as usize;

            let result = tokio::task::spawn_blocking(move || {
                use crate::signing::solana::{
                    initialize_match_ix, make_rpc, sign_and_submit, start_tournament_ix,
                };
                use solana_sdk::pubkey::Pubkey;
                use solana_sdk::signature::Signer;
                use std::str::FromStr;

                let program_id = Pubkey::from_str(&program_id_str)
                    .map_err(|e| format!("bad program_id: {e}"))?;
                let rpc = make_rpc(&rpc_url);

                // Tx 1: start_tournament
                let ix = start_tournament_ix(
                    &program_id,
                    tournament_id,
                    &authority.pubkey(),
                    &host_treasury,
                );
                sign_and_submit(&rpc, &authority, &[ix])
                    .map_err(|e| format!("start_tournament tx: {e}"))?;

                // Tx batches: initialize_match (20 per batch)
                let mut idx = 0u16;
                while (idx as usize) < total_matches {
                    let end = ((idx as usize + 20).min(total_matches)) as u16;
                    let ixs: Vec<_> = (idx..end)
                        .map(|i| {
                            let round = (i as f32 + 1.0).log2() as u8;
                            let next = if i == 0 { None } else { Some((i - 1) / 2) };
                            initialize_match_ix(
                                &program_id,
                                tournament_id,
                                i,
                                round,
                                None,
                                None,
                                next,
                                (i % 2) as u8,
                                &authority.pubkey(),
                            )
                        })
                        .collect();
                    sign_and_submit(&rpc, &authority, &ixs)
                        .map_err(|e| format!("initialize_match batch {idx}: {e}"))?;
                    idx = end;
                }
                Ok::<(), String>(())
            })
            .await;

            match result {
                Ok(Ok(())) => info!(
                    "[tournament-scheduler] On-chain start confirmed for tournament {}",
                    tournament_id
                ),
                Ok(Err(e)) => {
                    error!(
                        "[tournament-scheduler] On-chain start failed for tournament {}: {}",
                        tournament_id, e
                    );
                    return;
                }
                Err(e) => {
                    error!(
                        "[tournament-scheduler] spawn_blocking panicked for tournament {}: {}",
                        tournament_id, e
                    );
                    return;
                }
            }
        }

        // ── Store update ──────────────────────────────────────────────────────
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
        let msg = SwissMessage::BracketFired {
            tournament_id,
            player_count,
            started_at,
        };
        match serde_json::to_vec(&msg) {
            Ok(bytes) => {
                if let Err(e) = sender.broadcast(bytes.into()).await {
                    warn!(
                        "[tournament-scheduler] BracketFired broadcast failed for {}: {}",
                        tournament_id, e
                    );
                } else {
                    info!(
                        "[tournament-scheduler] BracketFired broadcast sent for tournament {}",
                        tournament_id
                    );
                }
            }
            Err(e) => warn!("[tournament-scheduler] BracketFired serialize error: {}", e),
        }
    }
}

/// Spawn the scheduler as a background task.
///
/// Pass `gossip` so the scheduler can broadcast [`SwissMessage::BracketFired`].
/// Pass `on_chain` so the scheduler fires on-chain txs when starting a tournament.
pub fn spawn_tournament_scheduler(
    store: TournamentStore,
    gossip: Option<Arc<TournamentGossipService>>,
    on_chain: Option<(String, String, Arc<Keypair>, solana_sdk::pubkey::Pubkey)>,
) -> mpsc::Sender<TournamentTrigger> {
    let (mut scheduler, trigger_tx) = TournamentScheduler::new(store.clone());
    if let Some(g) = gossip {
        scheduler.set_gossip(g);
    }
    if let Some((program_id, rpc_url, authority, host_treasury)) = on_chain {
        scheduler.set_on_chain(program_id, rpc_url, authority, host_treasury);
    }
    tokio::spawn(scheduler.run());
    spawn_scheduled_start_ticker(store, trigger_tx.clone());
    trigger_tx
}

/// Tick interval for the prize-distribution scanner.
const PRIZE_DISTRIBUTION_TICK: Duration = Duration::from_secs(60);

/// After completion, hold distribution this long while anti-cheat analysis of
/// the tournament's games is still pending. Past the window we pay anyway —
/// analysis lag must not freeze payouts indefinitely.
const PRIZE_HOLD_WINDOW_SECS: i64 = 15 * 60;

/// Anti-cheat gate decision for a completed tournament.
enum PrizeGate {
    /// Analysis still pending and we're inside the hold window.
    Hold,
    /// Distribute, withholding the places held by these flagged wallets.
    Proceed { flagged: Vec<String> },
}

/// Checks the anti-cheat queue and verdicts for a tournament's games.
async fn anticheat_gate(
    pool: &sqlx::SqlitePool,
    t: &crate::signing::storage::tournament::TournamentRecord,
) -> PrizeGate {
    let game_ids: Vec<String> = t
        .matches
        .iter()
        .flatten()
        .filter_map(|m| m.game_id)
        .map(|g| g.to_string())
        .collect();
    if game_ids.is_empty() {
        return PrizeGate::Proceed {
            flagged: Vec::new(),
        };
    }

    let placeholders = vec!["?"; game_ids.len()].join(",");

    // Any of this tournament's games still awaiting analysis?
    let pending: i64 = {
        let sql = format!("SELECT COUNT(*) FROM anticheat_queue WHERE game_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for id in &game_ids {
            q = q.bind(id);
        }
        q.fetch_one(pool).await.map(|(n,)| n).unwrap_or(0)
    };
    let now = chrono::Utc::now().timestamp();
    let within_window = t
        .completed_at
        .map(|c| now < c + PRIZE_HOLD_WINDOW_SECS)
        .unwrap_or(false);
    if pending > 0 && within_window {
        return PrizeGate::Hold;
    }

    // Collect wallets with a Flag verdict in any of this tournament's games.
    let flagged: Vec<String> = {
        let sql = format!(
            "SELECT white_pubkey, black_pubkey, white_verdict, black_verdict
             FROM anticheat_verdicts WHERE game_id IN ({placeholders})
             AND (white_verdict = 'Flag' OR black_verdict = 'Flag')"
        );
        let mut q = sqlx::query_as::<_, (String, String, String, String)>(&sql);
        for id in &game_ids {
            q = q.bind(id);
        }
        q.fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(w, b, wv, bv)| {
                let mut out = Vec::new();
                if wv == "Flag" {
                    out.push(w);
                }
                if bv == "Flag" {
                    out.push(b);
                }
                out
            })
            .collect()
    };
    PrizeGate::Proceed { flagged }
}

/// Spawns a background task that pushes SOL prizes to tournament winners as
/// soon as a tournament completes — winners never sign a claim transaction.
///
/// Scans for `Completed` tournaments with an unpaid prize pool and cranks the
/// permissionless `distribute_tournament_prizes` instruction. The instruction
/// is idempotent (claim bits guard double-pays), so retrying after a partial
/// failure is safe.
///
/// Distribution is gated on anti-cheat: held up to [`PRIZE_HOLD_WINDOW_SECS`]
/// while analysis of the tournament's games is pending, and places whose
/// winner has a `Flag` verdict are withheld (resolution goes through the
/// on-chain governance dispute flow).
pub fn spawn_prize_distributor(
    store: TournamentStore,
    pool: sqlx::SqlitePool,
    on_chain: Option<(String, String, Arc<Keypair>)>,
) {
    use crate::telemetry::worker_metrics;
    use std::sync::atomic::Ordering;

    let Some((program_id_str, rpc_url, authority)) = on_chain else {
        info!("[prize-distributor] No on-chain config — distributor not started");
        return;
    };
    tokio::spawn(async move {
        info!(
            "[prize-distributor] Auto prize distribution: {}s interval",
            PRIZE_DISTRIBUTION_TICK.as_secs()
        );
        let mut ticker = tokio::time::interval(PRIZE_DISTRIBUTION_TICK);
        ticker.tick().await; // skip immediate first tick

        loop {
            ticker.tick().await;
            for t in store.list().await {
                if t.status != TournamentStatus::Completed
                    || t.prizes_distributed
                    || t.prize_pool == 0
                    || t.winner.is_none()
                {
                    continue;
                }

                let flagged = match anticheat_gate(&pool, &t).await {
                    PrizeGate::Hold => {
                        worker_metrics::PRIZE_DISTRIBUTION_HELD_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                        info!(
                            "[prize-distributor] Tournament {} held — anti-cheat analysis pending",
                            t.tournament_id
                        );
                        continue;
                    }
                    PrizeGate::Proceed { flagged } => flagged,
                };

                use solana_sdk::pubkey::Pubkey;
                use std::str::FromStr;
                let places = [
                    &t.winner,
                    &t.second_place,
                    &t.third_place,
                    &t.fourth_place,
                    &t.fifth_place,
                    &t.sixth_place,
                    &t.seventh_place,
                    &t.eighth_place,
                    &t.ninth_place,
                    &t.tenth_place,
                ];
                let flagged_places = places
                    .iter()
                    .filter_map(|p| p.as_deref())
                    .filter(|s| flagged.iter().any(|f| f == s))
                    .count() as u64;
                if flagged_places > 0 {
                    worker_metrics::PRIZE_DISTRIBUTION_FLAGGED_TOTAL
                        .fetch_add(flagged_places, Ordering::Relaxed);
                    warn!(
                        "[prize-distributor] Tournament {}: withholding {} flagged prize place(s) — \
                         resolve via governance dispute",
                        t.tournament_id, flagged_places
                    );
                }
                let winners: Vec<Pubkey> = places
                    .iter()
                    .filter_map(|p| p.as_deref())
                    .filter(|s| !flagged.iter().any(|f| f == s))
                    .filter_map(|s| Pubkey::from_str(s).ok())
                    .collect();
                if winners.is_empty() {
                    // Every payable place is flagged — stop retrying; the
                    // escrow stays claimable through governance resolution.
                    warn!(
                        "[prize-distributor] Tournament {}: all prize places flagged, leaving escrow for governance",
                        t.tournament_id
                    );
                    store
                        .update(t.tournament_id, |t| t.prizes_distributed = true)
                        .await;
                    continue;
                }

                let tournament_id = t.tournament_id;
                let program_id_str = program_id_str.clone();
                let rpc_url = rpc_url.clone();
                let authority = authority.clone();
                let result = tokio::task::spawn_blocking(move || {
                    use crate::signing::solana::{
                        distribute_tournament_prizes_ix, make_rpc, sign_and_submit,
                    };
                    use solana_sdk::signature::Signer;

                    let program_id = Pubkey::from_str(&program_id_str)
                        .map_err(|e| format!("bad program_id: {e}"))?;
                    let rpc = make_rpc(&rpc_url);
                    let ix = distribute_tournament_prizes_ix(
                        &program_id,
                        tournament_id,
                        &authority.pubkey(),
                        &winners,
                    );
                    sign_and_submit(&rpc, &authority, &[ix])
                        .map_err(|e| format!("distribute tx: {e}"))
                })
                .await;

                match result {
                    Ok(Ok(sig)) => {
                        worker_metrics::PRIZE_DISTRIBUTED_TOTAL.fetch_add(1, Ordering::Relaxed);
                        info!(
                            "[prize-distributor] Tournament {} prizes distributed, sig {}",
                            tournament_id, sig
                        );
                        store
                            .update(tournament_id, |t| t.prizes_distributed = true)
                            .await;
                    }
                    Ok(Err(e)) => warn!(
                        "[prize-distributor] Tournament {} distribution failed (will retry): {}",
                        tournament_id, e
                    ),
                    Err(e) => error!(
                        "[prize-distributor] spawn_blocking panicked for tournament {}: {}",
                        tournament_id, e
                    ),
                }
            }
        }
    });
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
                let Some(scheduled_at) = t.scheduled_at else {
                    continue;
                };
                if now < scheduled_at {
                    continue;
                }
                let trigger = TournamentTrigger::ScheduledStart {
                    tournament_id: t.tournament_id,
                };
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
