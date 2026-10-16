use crate::signing::storage::tournament::{TournamentRecord, TournamentStatus, TournamentStore};
use crate::signing::swiss::{SwissService, SwissServiceError};
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use xfchess_braid_server::{bridge, ResourceHub};

pub const TOURNAMENT_TRIGGER_CHANNEL_SIZE: usize = 256;

/// Players needed before the scheduler may start the event. The program only starts
/// a single-elimination bracket when it is completely full, so a partial bracket is
/// never a startable state, whatever `min_players` says.
fn default_min_players(tournament: &TournamentRecord) -> usize {
    if !tournament.is_swiss() {
        return tournament.max_players as usize;
    }
    tournament
        .min_players
        .unwrap_or_else(|| tournament.max_players.min(8)) as usize
}

#[derive(Debug, Clone)]
pub enum TournamentTrigger {
    CheckStart {
        tournament_id: u64,
    },
    PlayerJoined {
        tournament_id: u64,
        player_count: usize,
    },
    AdminStart {
        tournament_id: u64,
    },
    ScheduledStart {
        tournament_id: u64,
    },
    FillGraceExpired {
        tournament_id: u64,
    },
}

pub struct TournamentScheduler {
    store: TournamentStore,
    trigger_tx: mpsc::Sender<TournamentTrigger>,
    trigger_rx: mpsc::Receiver<TournamentTrigger>,
    fill_timers: HashMap<u64, JoinHandle<()>>,
    braid_hub: Option<Arc<ResourceHub>>,
    swiss: Option<Arc<SwissService>>,
    on_chain: Option<OnChainConfig>,
}

struct OnChainConfig {
    program_id: String,
    rpc_url: String,
    vps_authority: Arc<Keypair>,
    host_treasury: solana_sdk::pubkey::Pubkey,
}

impl TournamentScheduler {
    fn cancellation_service(
        &self,
    ) -> Option<crate::signing::tournament_operations::CancellationService> {
        let config = self.on_chain.as_ref()?;
        Some(crate::signing::tournament_operations::CancellationService {
            store: self.store.clone(),
            rpc_url: config.rpc_url.clone(),
            program_id: config.program_id.parse().ok()?,
            authority: config.vps_authority.clone(),
            host_treasury: config.host_treasury,
        })
    }
    pub fn new(store: TournamentStore) -> (Self, mpsc::Sender<TournamentTrigger>) {
        let (trigger_tx, trigger_rx) = mpsc::channel(TOURNAMENT_TRIGGER_CHANNEL_SIZE);
        (
            Self {
                store,
                trigger_tx: trigger_tx.clone(),
                trigger_rx,
                fill_timers: HashMap::new(),
                braid_hub: None,
                swiss: None,
                on_chain: None,
            },
            trigger_tx,
        )
    }

    pub fn set_braid_hub(&mut self, hub: Arc<ResourceHub>) {
        self.braid_hub = Some(hub);
    }

    /// Lets the scheduler pair a Swiss event's first round when it starts one.
    pub fn set_swiss(&mut self, swiss: Arc<SwissService>) {
        self.swiss = Some(swiss);
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
        let mut reconciliation = tokio::time::interval(Duration::from_secs(30));
        loop {
            let trigger = tokio::select! {
                trigger = self.trigger_rx.recv() => match trigger { Some(trigger) => trigger, None => break },
                _ = reconciliation.tick() => {
                    if let Some(service) = self.cancellation_service() {
                        if let Err(e) = service.reconcile_pending().await {
                            error!("[tournament-scheduler] cancellation recovery failed: {}", e);
                        }
                    }
                    continue;
                }
            };
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

    const FILL_GRACE_SECS: u64 = 5 * 60;

    const GRACE_SECS: i64 = 10 * 60;

    fn abort_fill_timer(&mut self, tournament_id: u64) {
        if let Some(handle) = self.fill_timers.remove(&tournament_id) {
            handle.abort();
            info!(
                "[tournament-scheduler] Cancelled fill-grace timer for tournament {}",
                tournament_id
            );
        }
    }

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
        let min = default_min_players(&tournament);

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

        let min = default_min_players(&tournament);
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

        let min = default_min_players(&tournament);
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
                    if let Some(service) = self.cancellation_service() {
                        if let Err(e) = service
                            .cancel(
                                tournament_id,
                                "scheduler",
                                "Minimum player deadline expired",
                            )
                            .await
                        {
                            error!(
                                "[tournament-scheduler] cancellation pending for {}: {}",
                                tournament_id, e
                            );
                        }
                    } else {
                        warn!("[tournament-scheduler] cancellation requires chain configuration; keeping {} open", tournament_id);
                    }
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

    async fn start_tournament(&self, tournament_id: u64) {
        if let Ok(operations) =
            crate::signing::tournament_operations::list(self.store.pool(), tournament_id).await
        {
            if operations
                .iter()
                .any(|op| op.action == "cancel_tournament" && op.status != "failed")
            {
                return;
            }
        } else {
            return;
        }
        let Some(record) = self.store.get(tournament_id).await else {
            return;
        };
        if let Some(cfg) = &self.on_chain {
            // Draw the bracket the store will keep, so the chain gets the very same
            // one. A Swiss event has no bracket: its rounds are paired off-chain and
            // reported per board.
            let mut started = record.clone();
            if let Err(e) = started.start() {
                warn!(
                    "[tournament-scheduler] Tournament {} cannot start: {}",
                    tournament_id, e
                );
                return;
            }
            let bracket = started.elimination().cloned();
            let max_players = record.max_players;
            let program_id_str = cfg.program_id.clone();
            let rpc_url = cfg.rpc_url.clone();
            let authority = cfg.vps_authority.clone();
            let host_treasury = cfg.host_treasury;

            let result = tokio::task::spawn_blocking(move || {
                use crate::signing::solana::{make_rpc, sign_and_submit, start_tournament_ix};
                use solana_sdk::pubkey::Pubkey;
                use solana_sdk::signature::Signer;
                use std::str::FromStr;

                let program_id = Pubkey::from_str(&program_id_str)
                    .map_err(|e| format!("bad program_id: {e}"))?;
                let rpc = make_rpc(&rpc_url);

                let ix = start_tournament_ix(
                    &program_id,
                    tournament_id,
                    max_players,
                    &authority.pubkey(),
                    &host_treasury,
                );
                sign_and_submit(&rpc, &authority, &[ix])
                    .map_err(|e| format!("start_tournament tx: {e}"))?;
                if let Some(bracket) = bracket {
                    crate::signing::elimination::chain::initialize_matches(
                        &rpc,
                        &program_id,
                        &authority,
                        tournament_id,
                        &bracket,
                    )?;
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

        if let Err(e) = self.store.start_tournament(tournament_id).await {
            warn!(
                "[tournament-scheduler] Failed to start tournament {}: {}",
                tournament_id, e
            );
            return;
        }

        // A Swiss event plays as soon as round 1 is paired (the admin's
        // initialize-swiss route may have paired it already).
        if record.is_swiss() {
            match self.swiss.as_ref() {
                Some(swiss) => match swiss.start_round(tournament_id).await {
                    Ok(round) => info!(
                        "[tournament-scheduler] Tournament {} round {} paired",
                        tournament_id, round.round
                    ),
                    Err(SwissServiceError::RoundIncomplete(_)) => {}
                    Err(e) => error!(
                        "[tournament-scheduler] Tournament {} round 1 not paired: {}",
                        tournament_id, e
                    ),
                },
                None => warn!(
                    "[tournament-scheduler] No Swiss service — tournament {} round 1 not paired",
                    tournament_id
                ),
            }
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

        // Publish the started state as a durable resource so late subscribers see it.
        let Some(hub) = &self.braid_hub else {
            warn!(
                "[tournament-scheduler] No Braid hub — tournament {} start not published",
                tournament_id
            );
            return;
        };
        bridge::push_bracket_fired(hub, tournament_id, player_count, started_at);
        info!(
            "[tournament-scheduler] Published bracket start for tournament {}",
            tournament_id
        );
    }
}

pub fn spawn_tournament_scheduler(
    store: TournamentStore,
    braid_hub: Option<Arc<ResourceHub>>,
    swiss: Option<Arc<SwissService>>,
    on_chain: Option<(String, String, Arc<Keypair>, solana_sdk::pubkey::Pubkey)>,
) -> mpsc::Sender<TournamentTrigger> {
    let (mut scheduler, trigger_tx) = TournamentScheduler::new(store.clone());
    if let Some(hub) = braid_hub {
        scheduler.set_braid_hub(hub);
    }
    if let Some(swiss) = swiss {
        scheduler.set_swiss(swiss);
    }
    if let Some((program_id, rpc_url, authority, host_treasury)) = on_chain {
        scheduler.set_on_chain(program_id, rpc_url, authority, host_treasury);
    }
    tokio::spawn(scheduler.run());
    spawn_scheduled_start_ticker(store, trigger_tx.clone());
    trigger_tx
}

const PRIZE_DISTRIBUTION_TICK: Duration = Duration::from_secs(60);

const PRIZE_HOLD_WINDOW_SECS: i64 = 15 * 60;

const DEFAULT_PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS: u64 = 5_000_000_000; // 5 SOL

fn prize_auto_release_threshold_lamports() -> u64 {
    std::env::var("PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS)
}

fn awaiting_prize_release_approval(prize_pool: u64, approved: bool, threshold: u64) -> bool {
    prize_pool > threshold && !approved
}

/// What one distributor pass did for a tournament.
enum Distribution {
    Paid(solana_sdk::signature::Signature),
    /// Winners claimed every funded place themselves.
    NothingOwed,
    /// This many funded places are flagged; the instruction cannot skip them.
    Withheld(usize),
}

enum PrizeGate {
    Hold,
    Proceed { flagged: Vec<String> },
}

async fn anticheat_gate(
    pool: &sqlx::SqlitePool,
    t: &crate::signing::storage::tournament::TournamentRecord,
) -> PrizeGate {
    // Every game of the event, either format, replays included.
    let game_ids: Vec<String> = t
        .state
        .game_ids(t.tournament_id)
        .into_iter()
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
        // SAFETY: the only dynamic part is `placeholders`, a run of literal
        // "?" bind markers sized to `game_ids.len()` — every actual value is
        // bound below, never interpolated into the SQL text.
        let mut q = sqlx::query_as::<_, (i64,)>(sqlx::AssertSqlSafe(sql));
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
        // SAFETY: same as above — only the "?" placeholder run is dynamic.
        let mut q = sqlx::query_as::<_, (String, String, String, String)>(sqlx::AssertSqlSafe(sql));
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
            worker_metrics::PRIZE_DISTRIBUTOR_LAST_TICK_UNIX.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                Ordering::Relaxed,
            );
            for t in store.awaiting_prizes().await {
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

                if awaiting_prize_release_approval(
                    t.prize_pool,
                    t.prize_release_approved,
                    prize_auto_release_threshold_lamports(),
                ) {
                    worker_metrics::PRIZE_DISTRIBUTION_AWAITING_APPROVAL_TOTAL
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(
                        "[prize-distributor] Tournament {} prize pool ({} lamports) exceeds the \
                         auto-release threshold — held pending POST /admin/tournament/{}/approve-prize-release",
                        t.tournament_id, t.prize_pool, t.tournament_id
                    );
                    continue;
                }

                let tournament_id = t.tournament_id;
                let program_id_str = program_id_str.clone();
                let rpc_url = rpc_url.clone();
                let authority = authority.clone();
                let result = tokio::task::spawn_blocking(move || {
                    use crate::signing::solana::tournament_account::fetch_prize_ledger;
                    use crate::signing::solana::{
                        distribute_tournament_prizes_ix, make_rpc, sign_and_submit,
                    };
                    use solana_sdk::pubkey::Pubkey;
                    use solana_sdk::signature::Signer;
                    use std::str::FromStr;

                    let program_id = Pubkey::from_str(&program_id_str)
                        .map_err(|e| format!("bad program_id: {e}"))?;
                    let rpc = make_rpc(&rpc_url);
                    // The program pays every funded, unclaimed place in one
                    // instruction and rejects any other wallet list, so the list
                    // comes from the chain: places, shares and claimed bits.
                    let wallets =
                        fetch_prize_ledger(&rpc, &program_id, tournament_id)?.unpaid_wallets();
                    if wallets.is_empty() {
                        return Ok(Distribution::NothingOwed);
                    }
                    let flagged_places = wallets
                        .iter()
                        .filter(|w| flagged.contains(&w.to_string()))
                        .count();
                    if flagged_places > 0 {
                        return Ok(Distribution::Withheld(flagged_places));
                    }
                    let ix = distribute_tournament_prizes_ix(
                        &program_id,
                        tournament_id,
                        &authority.pubkey(),
                        &wallets,
                    );
                    sign_and_submit(&rpc, &authority, &[ix])
                        .map(Distribution::Paid)
                        .map_err(|e| format!("distribute tx: {e}"))
                })
                .await;

                match result {
                    Ok(Ok(Distribution::Paid(sig))) => {
                        worker_metrics::PRIZE_DISTRIBUTED_TOTAL.fetch_add(1, Ordering::Relaxed);
                        info!(
                            "[prize-distributor] Tournament {} prizes distributed, sig {}",
                            tournament_id, sig
                        );
                        store
                            .update(tournament_id, |t| t.prizes_distributed = true)
                            .await;
                    }
                    Ok(Ok(Distribution::NothingOwed)) => {
                        info!(
                            "[prize-distributor] Tournament {}: every funded place is already claimed",
                            tournament_id
                        );
                        store
                            .update(tournament_id, |t| t.prizes_distributed = true)
                            .await;
                    }
                    Ok(Ok(Distribution::Withheld(places))) => {
                        worker_metrics::PRIZE_DISTRIBUTION_FLAGGED_TOTAL
                            .fetch_add(places as u64, Ordering::Relaxed);
                        // Withholding is advisory: claim_tournament_prize is
                        // permissionless, so it does not stop a flagged winner
                        // who claims directly. See docs/development/backlog.md.
                        warn!(
                            "[prize-distributor] Tournament {}: {} funded place(s) flagged by \
                             anti-cheat — distribution withheld for governance review",
                            tournament_id, places
                        );
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

const SCHEDULED_START_TICK: Duration = Duration::from_secs(30);

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
            crate::telemetry::worker_metrics::TOURNAMENT_SCHEDULER_LAST_TICK_UNIX.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                std::sync::atomic::Ordering::Relaxed,
            );
            let now = chrono::Utc::now().timestamp();
            for t in store.with_status(TournamentStatus::Registration).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::storage::tournament::{
        EliminationState, FormatState, ResultSource, SwissStorageData, TournamentFormat,
    };

    #[test]
    fn a_bracket_starts_full_but_a_swiss_event_at_its_minimum() {
        let mut t = TournamentRecord::new(1, "t", 0);
        t.max_players = 16;
        assert_eq!(default_min_players(&t), 16);
        t.state = FormatState::for_format(&TournamentFormat::Swiss { rounds: 4 });
        assert_eq!(default_min_players(&t), 8);
        t.min_players = Some(5);
        assert_eq!(default_min_players(&t), 5);
    }

    async fn anticheat_migrated_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::infrastructure::migrate_session_db(&pool)
            .await
            .unwrap();
        pool
    }

    fn tournament_with_one_match(game_id: u64, completed_at: Option<i64>) -> TournamentRecord {
        let mut t = TournamentRecord::new(1, "t", 0);
        t.completed_at = completed_at;
        let mut bracket = EliminationState::draw(1, &["white".to_string(), "black".to_string()]);
        bracket.matches[0].game_id = Some(game_id);
        bracket
            .decide(1, 0, "white", "black", ResultSource::OnChain)
            .unwrap();
        t.state = FormatState::Elimination(bracket);
        t
    }

    #[tokio::test]
    async fn prize_gate_pays_unanalyzed_game_after_hold_window_expires() {
        let pool = anticheat_migrated_pool().await;
        let game_id = 555_u64;

        sqlx::query("INSERT INTO anticheat_queue (game_id) VALUES (?)")
            .bind(game_id.to_string())
            .execute(&pool)
            .await
            .unwrap();

        let now = chrono::Utc::now().timestamp();
        let t = tournament_with_one_match(game_id, Some(now - PRIZE_HOLD_WINDOW_SECS - 10));

        let gate = anticheat_gate(&pool, &t).await;
        assert!(
            matches!(gate, PrizeGate::Proceed { flagged } if flagged.is_empty()),
            "past the hold window, an unanalyzed game must pay out (Proceed), not Hold forever"
        );
    }

    #[tokio::test]
    async fn prize_gate_holds_unanalyzed_game_within_hold_window() {
        let pool = anticheat_migrated_pool().await;
        let game_id = 556_u64;

        sqlx::query("INSERT INTO anticheat_queue (game_id) VALUES (?)")
            .bind(game_id.to_string())
            .execute(&pool)
            .await
            .unwrap();

        let now = chrono::Utc::now().timestamp();
        let t = tournament_with_one_match(game_id, Some(now - 60)); // 1 minute ago

        let gate = anticheat_gate(&pool, &t).await;
        assert!(
            matches!(gate, PrizeGate::Hold),
            "inside the hold window, pending analysis must hold distribution"
        );
    }

    #[test]
    fn small_prize_pool_never_needs_approval() {
        assert!(!awaiting_prize_release_approval(
            1_000_000,
            false,
            5_000_000_000
        ));
        assert!(!awaiting_prize_release_approval(
            1_000_000,
            true,
            5_000_000_000
        ));
    }

    #[test]
    fn large_unapproved_prize_pool_is_held() {
        assert!(awaiting_prize_release_approval(
            10_000_000_000,
            false,
            5_000_000_000
        ));
    }

    #[test]
    fn large_approved_prize_pool_proceeds() {
        assert!(!awaiting_prize_release_approval(
            10_000_000_000,
            true,
            5_000_000_000
        ));
    }

    #[test]
    fn threshold_boundary_is_exclusive() {
        // Exactly at the threshold: still auto-releases (only pools that
        // *exceed* the threshold need approval).
        assert!(!awaiting_prize_release_approval(
            5_000_000_000,
            false,
            5_000_000_000
        ));
        assert!(awaiting_prize_release_approval(
            5_000_000_001,
            false,
            5_000_000_000
        ));
    }

    static PRIZE_THRESHOLD_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn threshold_env_var_overrides_default() {
        let _guard = PRIZE_THRESHOLD_ENV_LOCK.lock().unwrap();
        std::env::set_var("PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS", "123");
        assert_eq!(prize_auto_release_threshold_lamports(), 123);
        std::env::remove_var("PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS");
        assert_eq!(
            prize_auto_release_threshold_lamports(),
            DEFAULT_PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS
        );
    }

    #[tokio::test]
    async fn flagged_verdict_is_invisible_outside_its_own_tournaments_gate_query() {
        let pool = anticheat_migrated_pool().await;
        let unrelated_game_id = "42";

        sqlx::query(
            "INSERT INTO anticheat_verdicts
             (game_id, white_pubkey, black_pubkey, white_verdict, black_verdict,
              white_score, black_score, white_signals, black_signals)
             VALUES (?, 'white_wallet', 'black_wallet', 'Flag', 'Clean', 0.9, 0.1, '{}', '{}')",
        )
        .bind(unrelated_game_id)
        .execute(&pool)
        .await
        .unwrap();

        // A different tournament whose matches never reference `unrelated_game_id`.
        let t = tournament_with_one_match(999, Some(0));
        let gate = anticheat_gate(&pool, &t).await;
        assert!(
            matches!(gate, PrizeGate::Proceed { flagged } if flagged.is_empty()),
            "a Flag verdict for a game outside this tournament's own matches must not surface here"
        );
    }

    #[tokio::test]
    async fn prize_gate_checks_swiss_board_games_too() {
        let pool = anticheat_migrated_pool().await;
        let mut sd = SwissStorageData::new(1);
        sd.current_round = 1;
        sd.rounds = vec![swiss_pairing::SwissRound {
            round: 1,
            pairings: vec![swiss_pairing::Pairing {
                white: "cheater".into(),
                black: "honest".into(),
                board: 1,
            }],
            byes: vec![],
            float_downs: vec![],
            float_ups: vec![],
        }];
        let mut t = TournamentRecord::new(3, "t", 0);
        t.state = FormatState::Swiss(sd);
        let board_game =
            crate::signing::storage::tournament::TournamentStore::swiss_game_id(3, 1, 1).unwrap();
        sqlx::query(
            "INSERT INTO anticheat_verdicts
             (game_id, white_pubkey, black_pubkey, white_verdict, black_verdict,
              white_score, black_score, white_signals, black_signals)
             VALUES (?, 'cheater', 'honest', 'Flag', 'Clean', 0.9, 0.1, '{}', '{}')",
        )
        .bind(board_game.to_string())
        .execute(&pool)
        .await
        .unwrap();

        let gate = anticheat_gate(&pool, &t).await;
        assert!(
            matches!(gate, PrizeGate::Proceed { ref flagged } if flagged == &["cheater"]),
            "a flagged Swiss game must withhold that player's prize"
        );
    }
}
