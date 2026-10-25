use crate::db::repository::GameRepository;
use crate::signing::anticheat_enqueue::{enqueue_game_analysis, FinalizedGame};
use crate::signing::solana::{self, GAME_SEED};
use crate::signing::storage::tournament::GameOutcome;
use crate::signing::AppState;
use crate::signing::OrchestratorEvent;
use crate::telemetry::worker_metrics;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use xfchess_client_compat::prelude::*;

const SETTLEMENT_TICK: Duration = Duration::from_secs(30);

const STALE_DELEGATION_SECS: i64 = 20 * 60;

const STALE_UNDELEGATED_SECS: i64 = 5 * 60;

// GameStatus / GameResult borsh tags now live with the shared decoder, so the
// worker and every other reader agree on them by construction.
use crate::signing::solana::game_account::{
    GameAccount as GameSnapshot, RESULT_NONE, STATUS_ACTIVE, STATUS_CANCELLED, STATUS_EXPIRED,
    STATUS_FINISHED, STATUS_SETTLED,
};

fn parse_game_account(data: &[u8]) -> Option<GameSnapshot> {
    crate::signing::solana::game_account::parse(data)
}

const RPC_BATCH_SIZE: usize = 100;

/// Wait this long before resolving a missing Game PDA, allowing RPC creation lag.
const MISSING_GAME_GRACE_SECS: i64 = 10 * 60;

/// Finalization closes the Game PDA; only rounding dust should remain in escrow.
/// A larger balance indicates an unsettled stake.
const ESCROW_DUST_LAMPORTS: u64 = 5_000;

#[derive(Debug, PartialEq, Eq)]
enum MissingGameResolution {
    /// Escrow is empty: settled-and-closed, or never funded. Safe to retire.
    Resolved,
    /// Escrow still holds funds with no Game account — operator recovery.
    FundsHeld(u64),
    /// Escrow balance or session age could not be established this tick.
    Unknown,
    /// Too recent to distinguish from RPC lag.
    Grace,
}

/// Finalization closes the Game PDA, but absence alone does not prove escrow was paid.
fn resolve_missing_game(
    escrow: &Fetched,
    session_created_at: Option<i64>,
    now: i64,
) -> MissingGameResolution {
    let Some(created_at) = session_created_at else {
        return MissingGameResolution::Unknown;
    };
    // created_at == 0 marks rows that predate the column: old enough.
    if created_at > 0 && now.saturating_sub(created_at) < MISSING_GAME_GRACE_SECS {
        return MissingGameResolution::Grace;
    }
    match escrow {
        Fetched::Unknown => MissingGameResolution::Unknown,
        Fetched::Missing => MissingGameResolution::Resolved,
        Fetched::Found(acc) if acc.lamports <= ESCROW_DUST_LAMPORTS => {
            MissingGameResolution::Resolved
        }
        Fetched::Found(acc) => MissingGameResolution::FundsHeld(acc.lamports),
    }
}

/// The last settle-ready snapshot of each game the worker has tried to
/// finalize, kept until the game's off-chain effects are recorded. It lets a
/// game whose PDA closes out from under the worker still be recorded.
type SettleReady = HashMap<u64, GameSnapshot>;

/// Whether a settlement path already recorded the game. A completed row with
/// no settlement signature is an off-chain (free-rated) result written by a
/// player and is not proof that settlement bookkeeping ran.
async fn already_recorded(state: &AppState, game_id: u64) -> bool {
    let repo = GameRepository::new(state.store.pool());
    matches!(
        repo.get_game(&game_id.to_string()).await,
        Ok(Some(record)) if record.status == "completed"
            && record.finalize_sig.as_deref().is_some_and(|sig| !sig.trim().is_empty())
    )
}

/// Retire sessions whose Game PDA is gone only when the escrow PDA confirms
/// the stake was paid out or refunded; otherwise keep them and report.
async fn reconcile_missing_games(
    state: &Arc<AppState>,
    program_id: &Pubkey,
    missing: Vec<u64>,
    settle_ready: &mut SettleReady,
    discovered: &mut BTreeSet<u64>,
    now: i64,
) -> u64 {
    if missing.is_empty() {
        return 0;
    }
    let escrows: Vec<Pubkey> = missing
        .iter()
        .map(|id| {
            Pubkey::find_program_address(
                &[solana::WAGER_ESCROW_SEED, &id.to_le_bytes()],
                program_id,
            )
            .0
        })
        .collect();
    let fetched = fetch_accounts_batched(
        state.config.solana_rpc_url.clone(),
        escrows,
        state.metrics.clone(),
    )
    .await;
    if fetched.len() != missing.len() {
        warn!("[settlement] escrow reconciliation returned the wrong account count");
        return 0;
    }
    let mut resolved = 0;
    for (game_id, escrow) in missing.into_iter().zip(fetched) {
        let created_at = match state.store.created_at(game_id).await {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "[settlement] game {}: session lookup failed: {}",
                    game_id, e
                );
                None
            }
        };
        let created_at = created_at.or_else(|| discovered.contains(&game_id).then_some(0));
        match resolve_missing_game(&escrow, created_at, now) {
            MissingGameResolution::Resolved => {
                discovered.remove(&game_id);
                info!(
                    "[settlement] game {}: Game PDA closed and escrow empty; retiring session",
                    game_id
                );
                match settle_ready.remove(&game_id) {
                    // Settled by another caller, or by our own transaction whose
                    // confirmation was lost: the result is known from the last
                    // snapshot, so the off-chain effects must still happen.
                    Some(snap) if !already_recorded(state, game_id).await => {
                        record_settled_game(state, game_id, &snap, None).await
                    }
                    _ => state.store.deactivate(game_id).await,
                }
                resolved += 1;
            }
            MissingGameResolution::FundsHeld(lamports) => error!(
                "[settlement] game {}: Game PDA missing but escrow still holds {} lamports — \
                 operator recovery required (see docs/runbooks/game-settlement.md)",
                game_id, lamports
            ),
            MissingGameResolution::Unknown | MissingGameResolution::Grace => {}
        }
    }
    resolved
}

enum Fetched {
    Unknown,
    Missing,
    Found(solana_sdk::account::Account),
}

async fn fetch_accounts_batched(
    rpc_url: String,
    pdas: Vec<Pubkey>,
    metrics: Arc<crate::telemetry::Metrics>,
) -> Vec<Fetched> {
    tokio::task::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        let rpc = match solana::make_rpc(&rpc_url) {
            Ok(rpc) => rpc,
            Err(error) => {
                warn!("Batched game lookup client unavailable: {error}");
                return std::iter::repeat_with(|| Fetched::Unknown)
                    .take(pdas.len())
                    .collect();
            }
        };
        let mut out = Vec::with_capacity(pdas.len());
        for chunk in pdas.chunks(RPC_BATCH_SIZE) {
            worker_metrics::SETTLEMENT_RPC_CALLS_TOTAL.fetch_add(1, Ordering::Relaxed);
            let started = std::time::Instant::now();
            let result = rpc.get_multiple_sdk_accounts(chunk);
            metrics.record_solana_rpc_call(
                "getMultipleAccounts",
                result.is_ok(),
                started.elapsed().as_millis() as f64,
            );
            match result {
                Ok(accounts) => out.extend(accounts.into_iter().map(|a| match a {
                    Some(acc) => Fetched::Found(acc),
                    None => Fetched::Missing,
                })),
                Err(e) => {
                    warn!(
                        "[settlement] batched fetch of {} accounts failed: {}",
                        chunk.len(),
                        e
                    );
                    out.extend(std::iter::repeat_with(|| Fetched::Unknown).take(chunk.len()));
                }
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

pub fn spawn_settlement_worker(state: Arc<AppState>) {
    tokio::spawn(async move {
        info!(
            "[settlement] Auto-settlement worker started ({}s interval)",
            SETTLEMENT_TICK.as_secs()
        );
        match reconcile_startup_sessions(&state).await {
            Ok(deactivated) if deactivated > 0 => info!(
                "[settlement] Startup reconciliation deactivated {} terminal session(s)",
                deactivated
            ),
            Ok(_) => info!("[settlement] Startup reconciliation found no terminal sessions"),
            Err(e) => warn!("[settlement] Startup reconciliation failed: {e}"),
        }
        let mut ticker = tokio::time::interval(SETTLEMENT_TICK);
        ticker.tick().await; // skip the immediate first tick
        let mut settle_ready = SettleReady::new();
        let mut discovered = BTreeSet::new();
        let mut last_discovery: Option<std::time::Instant> = None;

        loop {
            ticker.tick().await;
            let started = std::time::Instant::now();
            use std::sync::atomic::Ordering;
            worker_metrics::SETTLEMENT_LAST_TICK_UNIX.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                Ordering::Relaxed,
            );
            // On-chain games survive session database loss and expired keys.
            // Scan both owners periodically; keep the prior set if RPC fails.
            if last_discovery.is_none_or(|at| at.elapsed() >= Duration::from_secs(300)) {
                let rpc = state.solana_rpc.clone();
                let program = state.program_id;
                match tokio::task::spawn_blocking(move || {
                    solana::game_discovery::discover_game_ids(&rpc, &program)
                })
                .await
                {
                    Ok(Ok(ids)) => discovered.extend(ids),
                    result => warn!(
                        ?result,
                        "[settlement] on-chain game discovery incomplete; retaining candidates"
                    ),
                }
                last_discovery = Some(std::time::Instant::now());
            }
            let scanned = match run_tick(&state, &mut settle_ready, &mut discovered).await {
                Ok(n) => n,
                Err(e) => {
                    warn!("[settlement] tick failed: {e}");
                    0
                }
            };
            worker_metrics::SETTLEMENT_TICKS_TOTAL.fetch_add(1, Ordering::Relaxed);
            worker_metrics::SETTLEMENT_GAMES_SCANNED_TOTAL.fetch_add(scanned, Ordering::Relaxed);
            worker_metrics::SETTLEMENT_TICK_MILLIS
                .store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        }
    });
}

async fn reconcile_startup_sessions(state: &Arc<AppState>) -> Result<u64, String> {
    let game_ids = state
        .store
        .try_list_active_game_ids()
        .await
        .map_err(|e| format!("list active sessions: {e}"))?;
    if game_ids.is_empty() {
        return Ok(0);
    }

    let program_id =
        Pubkey::from_str(&state.config.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    let pdas: Vec<Pubkey> = game_ids
        .iter()
        .map(|id| Pubkey::find_program_address(&[GAME_SEED, &id.to_le_bytes()], &program_id).0)
        .collect();
    let fetched = fetch_accounts_batched(
        state.config.solana_rpc_url.clone(),
        pdas,
        state.metrics.clone(),
    )
    .await;
    if fetched.len() != game_ids.len() {
        return Err("startup reconciliation returned the wrong account count".into());
    }

    let mut deactivated = 0;
    let mut missing = Vec::new();
    for (game_id, fetched) in game_ids.into_iter().zip(fetched) {
        let terminal = match fetched {
            // A missing account is reconciled against its escrow below.
            Fetched::Missing => {
                missing.push(game_id);
                false
            }
            Fetched::Found(account) => parse_game_account(&account.data)
                .map(|snapshot| matches!(snapshot.status, STATUS_SETTLED | STATUS_EXPIRED))
                .unwrap_or(false),
            Fetched::Unknown => false,
        };
        if terminal {
            state.store.deactivate(game_id).await;
            deactivated += 1;
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    deactivated += reconcile_missing_games(
        state,
        &program_id,
        missing,
        &mut SettleReady::new(),
        &mut BTreeSet::new(),
        now,
    )
    .await;

    Ok(deactivated)
}

async fn run_tick(
    state: &Arc<AppState>,
    settle_ready: &mut SettleReady,
    discovered: &mut BTreeSet<u64>,
) -> Result<u64, String> {
    let mut game_ids = state
        .store
        .try_list_active_game_ids()
        .await
        .map_err(|e| format!("list active sessions: {e}"))?;
    game_ids.extend(discovered.iter().copied());
    game_ids.sort_unstable();
    game_ids.dedup();
    run_games(state, settle_ready, discovered, game_ids).await
}

async fn run_games(
    state: &Arc<AppState>,
    settle_ready: &mut SettleReady,
    discovered: &mut BTreeSet<u64>,
    game_ids: Vec<u64>,
) -> Result<u64, String> {
    if game_ids.is_empty() {
        return Ok(0);
    }
    let program_id =
        Pubkey::from_str(&state.config.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    let pdas: Vec<Pubkey> = game_ids
        .iter()
        .map(|id| Pubkey::find_program_address(&[GAME_SEED, &id.to_le_bytes()], &program_id).0)
        .collect();

    let fetched = fetch_accounts_batched(
        state.config.solana_rpc_url.clone(),
        pdas.clone(),
        state.metrics.clone(),
    )
    .await;
    if fetched.len() != game_ids.len() {
        return Err("batched fetch returned wrong length".into());
    }

    // Games whose base copy says they are delegated, with that last committed copy.
    let mut delegated: Vec<(usize, GameSnapshot)> = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut stale_delegated: u64 = 0;
    let mut missing: Vec<u64> = Vec::new();

    for (i, f) in fetched.iter().enumerate() {
        let game_id = game_ids[i];
        match f {
            Fetched::Unknown => {}
            Fetched::Missing => missing.push(game_id),
            Fetched::Found(account) => {
                let Some(snap) =
                    solana::game_discovery::checked_game(&program_id, &pdas[i], account)
                else {
                    warn!("[settlement] game {}: unparseable game account", game_id);
                    continue;
                };
                match snap.status {
                    STATUS_SETTLED | STATUS_EXPIRED => {
                        discovered.remove(&game_id);
                        state.store.deactivate(game_id).await;
                    }
                    // Unjoined cancellation already refunded the creator; finalize requires
                    // a nonexistent black profile and must not be retried.
                    STATUS_CANCELLED if !snap.is_delegated && snap.black == Pubkey::default() => {
                        discovered.remove(&game_id);
                        info!(
                            "[settlement] game {}: cancelled before an opponent joined; refund was part of the cancellation",
                            game_id
                        );
                        state.store.deactivate(game_id).await;
                    }
                    STATUS_CANCELLED | STATUS_FINISHED
                        if !snap.is_delegated
                            && account.owner == program_id
                            && (snap.status == STATUS_CANCELLED
                                || snap.result_tag != RESULT_NONE) =>
                    {
                        settle_ready.insert(game_id, snap.clone());
                        let _guard =
                            crate::signing::routes::main::er_game_lock(state, game_id).await;
                        match finalize_on_chain(state, game_id, state.feepayer.next(), &snap).await
                        {
                            Ok(()) => {
                                settle_ready.remove(&game_id);
                                discovered.remove(&game_id);
                            }
                            Err(e) => warn!("[settlement] game {}: {}", game_id, e),
                        }
                    }
                    _ if snap.is_delegated || account.owner != program_id => {
                        delegated.push((i, snap))
                    }
                    STATUS_ACTIVE if !snap.is_delegated && snap.wager_amount > 0 => {
                        let stale_for = now.saturating_sub(snap.updated_at);
                        if stale_for > STALE_UNDELEGATED_SECS {
                            warn!(
                                "[settlement] game {} is active, wagered, and still not \
                                 delegated to the ER after {}m — possible stuck/failed \
                                 delegation; attempting to redelegate",
                                game_id,
                                STALE_UNDELEGATED_SECS / 60
                            );
                            // Same fire-once-shortly-after-crossing pattern as the
                            // stale-delegated case above — not on every tick thereafter.
                            if stale_for
                                < STALE_UNDELEGATED_SECS + SETTLEMENT_TICK.as_secs() as i64 * 2
                            {
                                redelegate_stale_game(state, game_id, &program_id).await;
                            }
                        }
                    }
                    _ => {} // still in progress
                }
            }
        }
    }

    reconcile_missing_games(state, &program_id, missing, settle_ready, discovered, now).await;
    // Games retired by any other path no longer need a fallback snapshot.
    settle_ready.retain(|game_id, _| game_ids.contains(game_id));

    // The base copy only changes on commits; the live ER copy decides what a
    // delegated game needs.
    if !delegated.is_empty() {
        for (i, base) in &delegated {
            let game_id = game_ids[*i];
            let er_rpc = match solana::routing::er_rpc_for_game(&state.config, game_id).await {
                Ok(rpc) => rpc,
                Err(e) => {
                    // A routing failure is not proof that the owning ER is down.
                    // Never wipe a game because a configured region is wrong.
                    warn!("[settlement] game {game_id}: {e}; retaining escrow for retry");
                    continue;
                }
            };
            let er_url = er_rpc.url();
            let er =
                fetch_accounts_batched(er_url.clone(), vec![pdas[*i]], state.metrics.clone()).await;
            let Some(er) = er.into_iter().next() else {
                continue;
            };
            if matches!(er, Fetched::Unknown) {
                continue; // transport failure does not establish an unavailable game
            }
            let er_snap = match &er {
                Fetched::Found(acc) if acc.owner == program_id => {
                    solana::game_discovery::checked_game(&program_id, &pdas[*i], acc)
                }
                Fetched::Found(_) => None,
                Fetched::Missing | Fetched::Unknown => None,
            };
            if matches!(&er, Fetched::Found(_)) && er_snap.is_none() {
                warn!("[settlement] game {game_id}: invalid ER account; retaining escrow");
                continue;
            }
            if er_snap.as_ref().is_some_and(|live| !live.is_delegated) {
                continue; // undelegation is already awaiting its base-layer commit
            }
            match delegated_action(er_snap.as_ref(), now) {
                DelegatedAction::Undelegate => {
                    match undelegate_from_er(state, game_id, &program_id, &er_url).await {
                        Ok(()) => continue,
                        Err(e) => {
                            warn!("[settlement] game {}: {}", game_id, e);
                            continue;
                        }
                    }
                }
                DelegatedAction::ClaimTimeout => {
                    if let Err(e) = claim_timeout_on_er(state, game_id, &program_id, &er_url).await
                    {
                        warn!("[settlement] game {game_id}: {e}");
                    }
                    continue;
                }
                DelegatedAction::Wait => continue,
                DelegatedAction::CheckErHealth => {}
            }
            // Here the ER has no live copy of the game, or cannot return a finished
            // one. Only then, and only once the committed copy has gone quiet, is
            // the wipe-and-refund escape hatch appropriate (MAGICBLOCK.md).
            if matches!(&er, Fetched::Missing)
                && now.saturating_sub(base.updated_at) > STALE_DELEGATION_SECS
            {
                stale_delegated += 1;
                warn!(
                    "[settlement] game {} is delegated but the ER cannot serve or return it \
                     and nothing was committed for over {}m — driving forced undelegation",
                    game_id,
                    STALE_DELEGATION_SECS / 60
                );
                drive_forced_undelegation(state, game_id, &program_id, base.white, base.black)
                    .await;
            }
        }
    }

    worker_metrics::SETTLEMENT_STALE_DELEGATED_GAUGE
        .store(stale_delegated, std::sync::atomic::Ordering::Relaxed);

    Ok(game_ids.len() as u64)
}

async fn undelegate_from_er(
    state: &Arc<AppState>,
    game_id: u64,
    program_id: &Pubkey,
    er_url: &str,
) -> Result<(), String> {
    let _guard = crate::signing::routes::main::er_game_lock(state, game_id).await;
    // Terminal undelegation is permissionless. Its transaction sponsor need not
    // be the game's expired or missing session key.
    let session_kp =
        solana_sdk::signature::Keypair::try_from(state.feepayer.next().to_bytes().as_slice())
            .map_err(|e| format!("Invalid recovery sponsor: {e}"))?;
    let session_pk = session_kp.pubkey();
    let ix = solana::undelegate_game_ix(program_id, &session_pk, game_id)
        .map_err(|e| format!("build undelegate: {e}"))?;
    let er_url = er_url.to_string();
    let submit_started = std::time::Instant::now();
    state.metrics.record_transaction_submitted("er");
    let submit_url = er_url.clone();
    let sig_result = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&submit_url)?;
        solana::sign_and_submit_er(&rpc, &session_kp, &[ix])
    })
    .await
    .map_err(|e| format!("join error: {e}"))?;
    let sig = match sig_result {
        Ok(sig) => {
            state
                .metrics
                .record_transaction_confirmed("er", submit_started.elapsed().as_millis() as f64);
            sig
        }
        Err(e) => {
            let category = solana::classify_error_str(&e.to_string()).to_string();
            state.metrics.record_transaction_failed("er", &category);
            return Err(format!("undelegate: {e}"));
        }
    };
    worker_metrics::SETTLEMENT_UNDELEGATED_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    info!(
        "[settlement] game {} finished on ER — undelegated, sig {}",
        game_id, sig
    );

    // Crank cancellation is best effort and must not change the recorded undelegation result.
    let cancel_kp =
        solana_sdk::signature::Keypair::try_from(state.feepayer.next().to_bytes().as_slice())
            .map_err(|e| format!("Invalid recovery sponsor: {e}"))?;
    let cancel_pk = cancel_kp.pubkey();
    let cancel_program_id = *program_id;
    match solana::cancel_time_check_ix(&cancel_program_id, &cancel_pk, game_id) {
        Ok(cancel_ix) => {
            let cancel_result = tokio::task::spawn_blocking(move || {
                let rpc = solana::make_rpc(&er_url)?;
                solana::sign_and_submit_er(&rpc, &cancel_kp, &[cancel_ix])
            })
            .await;
            match cancel_result {
                Ok(Ok(sig)) => {
                    info!(
                        "[settlement] game {} cancel_time_check sig {}",
                        game_id, sig
                    );
                    worker_metrics::TIME_CHECK_CANCELLED_TOTAL
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(Err(e)) => {
                    warn!(
                        "[settlement] cancel_time_check failed for game {}: {e}",
                        game_id
                    );
                    worker_metrics::TIME_CHECK_CANCEL_FAILED_TOTAL
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Err(e) => {
                    warn!(
                        "[settlement] cancel_time_check join error for game {}: {e}",
                        game_id
                    );
                    worker_metrics::TIME_CHECK_CANCEL_FAILED_TOTAL
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
        Err(e) => {
            warn!(
                "[settlement] Failed to build cancel_time_check instruction for game {}: {}",
                game_id, e
            );
            worker_metrics::TIME_CHECK_CANCEL_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    Ok(())
}

async fn redelegate_stale_game(state: &Arc<AppState>, game_id: u64, program_id: &Pubkey) {
    let entry = match state.store.get(game_id).await {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            warn!("Session missing for game {game_id}");
            return;
        }
        Err(error) => {
            warn!("Session unavailable for game {game_id}: {error}");
            return;
        }
    };
    let session_kp = entry.keypair();
    let session_pk = session_kp.pubkey();
    // Call .next() exactly once — it's round-robin, so a second call would
    // return a different keypair than the one baked into the instruction.
    let payer = state.feepayer.next();
    let payer_pk = payer.pubkey();

    let ix = match solana::delegate_game_ix(program_id, game_id, &payer_pk, &session_pk) {
        Ok(ix) => ix,
        Err(e) => {
            warn!(
                "[settlement] game {}: build delegate_game (redelegate): {}",
                game_id, e
            );
            worker_metrics::SETTLEMENT_REDELEGATE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::DelegateGame);
    let payer_bytes = payer.to_bytes();
    let session_bytes = session_kp.to_bytes();
    let result = tokio::task::spawn_blocking(move || {
        let payer_kp = solana_sdk::signature::Keypair::try_from(payer_bytes.as_slice())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let session_kp = solana_sdk::signature::Keypair::try_from(session_bytes.as_slice())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let rpc =
            solana::make_rpc(&rpc_url).map_err(|error| std::io::Error::other(error.to_string()))?;
        let blockhash = rpc.get_latest_blockhash()?;
        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_kp.pubkey()),
            &[&payer_kp, &session_kp],
            blockhash,
        );
        rpc.send_sdk_and_confirm_transaction(&tx)
    })
    .await;

    match result {
        Ok(Ok(sig)) => {
            info!("[settlement] game {} redelegated, sig {}", game_id, sig);
            solana::routing::invalidate_er_for_game(&state.config, game_id);
            worker_metrics::SETTLEMENT_REDELEGATE_RETRIED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            crate::signing::routes::main::schedule_time_check_crank(
                state,
                &state.config.solana_rpc_url,
                program_id,
                &session_kp,
                game_id,
            )
            .await;
        }
        Ok(Err(e)) => {
            warn!("[settlement] game {} redelegate failed: {e}", game_id);
            worker_metrics::SETTLEMENT_REDELEGATE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Err(e) => {
            warn!("[settlement] game {} redelegate join error: {e}", game_id);
            worker_metrics::SETTLEMENT_REDELEGATE_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

async fn request_force_undelegate_for_stale_game(
    state: &Arc<AppState>,
    game_id: u64,
    program_id: &Pubkey,
) {
    for payer in state.feepayer.all() {
        let ix = match solana::request_force_undelegate_ix(program_id, game_id, &payer.pubkey()) {
            Ok(ix) => ix,
            Err(e) => {
                warn!(
                    "[settlement] game {}: build request_force_undelegate: {}",
                    game_id, e
                );
                return;
            }
        };
        let rpc_url =
            solana::rpc_url_for(&state.config, solana::RoutedInstr::RequestForceUndelegate);
        let payer_bytes = payer.to_bytes();
        let result = tokio::task::spawn_blocking(move || {
            let kp = solana_sdk::signature::Keypair::try_from(payer_bytes.as_slice())
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let rpc = solana::make_rpc(&rpc_url)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            solana::sign_and_submit(&rpc, &kp, &[ix])
        })
        .await;
        match result {
            Ok(Ok(sig)) => {
                info!(
                    "[settlement] game {} force-undelegation requested (ER unavailability \
                     escape hatch) — recoverable without the ER in ~60min, sig {}",
                    game_id, sig
                );
                return;
            }
            Ok(Err(_)) => continue, // wrong payer for this game — try the next pool key
            Err(e) => {
                warn!(
                    "[settlement] game {}: request_force_undelegate join error: {}",
                    game_id, e
                );
                return;
            }
        }
    }
    warn!(
        "[settlement] game {}: request_force_undelegate failed with every fee-payer-pool key \
         (or none is the correct delegation rent payer yet — harmless, retried next staleness tick)",
        game_id
    );
}

fn parse_undelegation_request_expiry(data: &[u8]) -> Option<u64> {
    let o = 8 + 32;
    Some(u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?))
}

/// What a delegated game needs, judged from its live ER copy.
#[derive(Debug, PartialEq, Eq)]
enum DelegatedAction {
    /// The game has a terminal result: return it to base for settlement.
    Undelegate,
    /// The side to move has been idle past its whole clock: end it on time.
    ClaimTimeout,
    /// The ER is serving a game still in play.
    Wait,
    /// No usable ER copy: the ER may have stopped serving the game.
    CheckErHealth,
}

fn delegated_action(er: Option<&GameSnapshot>, now: i64) -> DelegatedAction {
    let Some(er) = er else {
        return DelegatedAction::CheckErHealth;
    };
    // The ER has already run undelegate_game; the commit is in flight.
    if !er.is_delegated {
        return DelegatedAction::CheckErHealth;
    }
    match er.status {
        STATUS_FINISHED if er.result_tag != RESULT_NONE => DelegatedAction::Undelegate,
        // A zero-move timeout cancels; settlement refunds both stakes.
        STATUS_CANCELLED => DelegatedAction::Undelegate,
        STATUS_ACTIVE if now.saturating_sub(er.updated_at) > er.timeout_claimable_after_secs() => {
            DelegatedAction::ClaimTimeout
        }
        STATUS_ACTIVE => DelegatedAction::Wait,
        _ => DelegatedAction::CheckErHealth,
    }
}

/// Ends an abandoned game through the program's own timeout rule. Nothing
/// else does so while the ER is healthy: clients do not claim timeouts, and
/// without this an abandoned game would sit delegated until it looked like an
/// ER failure and was wiped and refunded 50/50.
async fn claim_timeout_on_er(
    state: &Arc<AppState>,
    game_id: u64,
    program_id: &Pubkey,
    er_url: &str,
) -> Result<(), String> {
    let _guard = crate::signing::routes::main::er_game_lock(state, game_id).await;
    let session_kp =
        match solana_sdk::signature::Keypair::try_from(state.feepayer.next().to_bytes().as_slice())
        {
            Ok(kp) => kp,
            Err(e) => return Err(format!("Recovery sponsor unavailable: {e}")),
        };
    let ix = solana::claim_timeout_ix(program_id, &session_kp.pubkey(), game_id);
    let er_url = er_url.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&er_url)?;
        solana::sign_and_submit_er(&rpc, &session_kp, &[ix])
    })
    .await;
    match result {
        Ok(Ok(sig)) => {
            info!(
                "[settlement] game {} ended on time via claim_timeout, sig {}",
                game_id, sig
            );
            worker_metrics::SETTLEMENT_TIMEOUT_CLAIMED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
        Ok(Err(e)) => {
            warn!("[settlement] game {}: claim_timeout failed: {}", game_id, e);
            worker_metrics::SETTLEMENT_TIMEOUT_CLAIM_FAILED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Err(format!("Timeout claim failed: {e}"))
        }
        Err(e) => Err(format!("Timeout claim task failed: {e}")),
    }
}

/// User-triggered recovery uses only current chain state and permissionless
/// terminal instructions. It does not need a session database entry or key.
/// The HTTP handler authenticates the caller before invoking this function.
pub async fn recover_game(
    state: &Arc<AppState>,
    game_id: u64,
    wallet: Pubkey,
) -> Result<String, String> {
    let program = state.program_id;
    let pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program).0;
    let rpc = state.solana_rpc.clone();
    let account = tokio::task::spawn_blocking(move || rpc.get_sdk_account(&pda))
        .await
        .map_err(|e| format!("Chain lookup task failed: {e}"))?
        .map_err(|e| format!("Could not read game: {e}"))?;
    let game = solana::game_discovery::checked_game(&program, &pda, &account)
        .ok_or("Invalid game account")?;
    if wallet != game.white && (wallet != game.black || wallet == Pubkey::default()) {
        return Err("This wallet is not a player in this game".into());
    }
    if game.is_delegated || account.owner != program {
        let er_rpc = solana::routing::er_rpc_for_game(&state.config, game_id).await?;
        let er_url = er_rpc.url();
        let er_account = tokio::task::spawn_blocking(move || er_rpc.get_sdk_account(&pda))
            .await
            .map_err(|e| format!("ER lookup task failed: {e}"))?
            .map_err(|e| format!("Could not read live game: {e}"))?;
        if er_account.owner != program {
            return Err("Game is returning to the base layer; check again shortly".into());
        }
        let live = solana::game_discovery::checked_game(&program, &pda, &er_account)
            .ok_or("Invalid live game account")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as i64;
        match delegated_action(Some(&live), now) {
            DelegatedAction::ClaimTimeout => {
                claim_timeout_on_er(state, game_id, &program, &er_url).await?;
            }
            DelegatedAction::Undelegate => {}
            DelegatedAction::Wait => {
                return Err("This game is still in progress; its clock has not expired".into())
            }
            DelegatedAction::CheckErHealth => {
                return Err("Game is awaiting its base-layer commit; check again shortly".into())
            }
        }
        undelegate_from_er(state, game_id, &program, &er_url).await?;
        return Ok("Game ended. Return to the base layer requested; settlement will follow after confirmation.".into());
    }
    if matches!(game.status, STATUS_CANCELLED | STATUS_FINISHED) && game.black != Pubkey::default()
    {
        let _guard = crate::signing::routes::main::er_game_lock(state, game_id).await;
        finalize_on_chain(state, game_id, state.feepayer.next(), &game).await?;
        return Ok("Settlement confirmed on the base layer.".into());
    }
    Err("No terminal settlement is available for this game yet".into())
}

/// Drives the ER-unavailability escape hatch one step per tick: request a
/// forced undelegation when none exists, and complete it (wiping the Game
/// account) once the request has expired. Both instructions are idempotent.
async fn drive_forced_undelegation(
    state: &Arc<AppState>,
    game_id: u64,
    program_id: &Pubkey,
    white: Pubkey,
    black: Pubkey,
) {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let request_pda = Pubkey::new_from_array(
        ephemeral_rollups_sdk::pda::undelegation_request_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        )
        .to_bytes(),
    );

    let rpc_url = state.config.solana_rpc_url.clone();
    let lookup = tokio::task::spawn_blocking(move || {
        let rpc = match solana::make_rpc(&rpc_url) {
            Ok(rpc) => rpc,
            Err(error) => return (Err(error), None),
        };
        let request = rpc
            .get_account_with_commitment(&request_pda, rpc.commitment())
            .map(|response| response.value);
        (request, rpc.get_slot().ok())
    })
    .await;
    let (account, current_slot) = match lookup {
        Ok((Ok(account), Some(slot))) => (account, slot),
        // RPC hiccup: an absent request cannot be told apart, so retry next tick.
        Ok(_) => return,
        Err(e) => {
            warn!(
                "[settlement] game {}: request-expiry check join error: {}",
                game_id, e
            );
            return;
        }
    };
    let Some(account) = account else {
        request_force_undelegate_for_stale_game(state, game_id, program_id).await;
        return;
    };
    let Some(expires_at_slot) = parse_undelegation_request_expiry(&account.data) else {
        warn!(
            "[settlement] game {}: unparseable undelegation request account",
            game_id
        );
        return;
    };
    if current_slot < expires_at_slot {
        return; // window hasn't elapsed yet
    }

    for payer in state.feepayer.all() {
        let ix =
            match solana::force_undelegate_after_timeout_ix(program_id, game_id, &payer.pubkey()) {
                Ok(ix) => ix,
                Err(e) => {
                    warn!(
                        "[settlement] game {}: build force_undelegate_after_timeout: {}",
                        game_id, e
                    );
                    return;
                }
            };
        let rpc_url = solana::rpc_url_for(
            &state.config,
            solana::RoutedInstr::ForceUndelegateAfterTimeout,
        );
        let payer_bytes = payer.to_bytes();
        let result = tokio::task::spawn_blocking(move || {
            let kp = solana_sdk::signature::Keypair::try_from(payer_bytes.as_slice())
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let rpc = solana::make_rpc(&rpc_url)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            solana::sign_and_submit(&rpc, &kp, &[ix])
        })
        .await;
        match result {
            Ok(Ok(sig)) => {
                warn!(
                    "[settlement] game {} force-undelegated after timeout — Game PDA is now \
                     wiped and owned by our program again, sig {}; attempting automatic \
                     escrow release via recover_stuck_delegation",
                    game_id, sig
                );
                auto_recover_stuck_delegation(state, game_id, program_id, white, black).await;
                return;
            }
            Ok(Err(_)) => continue,
            Err(e) => {
                warn!(
                    "[settlement] game {}: force_undelegate_after_timeout join error: {}",
                    game_id, e
                );
                return;
            }
        }
    }
    warn!(
        "[settlement] game {}: force_undelegate_after_timeout failed with every fee-payer-pool key",
        game_id
    );
}

async fn auto_recover_stuck_delegation(
    state: &Arc<AppState>,
    game_id: u64,
    program_id: &Pubkey,
    white: Pubkey,
    black: Pubkey,
) {
    let Ok(authority_key) = std::env::var("DISPUTE_AUTHORITY_KEYPAIR") else {
        warn!(
            "[settlement] game {}: DISPUTE_AUTHORITY_KEYPAIR not set — cannot auto-recover \
             stuck-delegation escrow; falling back to manual admin route",
            game_id
        );
        worker_metrics::FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    };
    let authority = match crate::signing::load_keypair_from_env_value(&authority_key) {
        Ok(kp) => kp,
        Err(e) => {
            warn!(
                "[settlement] game {}: DISPUTE_AUTHORITY_KEYPAIR is set but invalid ({e}) — \
                 cannot auto-recover stuck-delegation escrow; falling back to manual admin route",
                game_id
            );
            worker_metrics::FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    let ix = solana::recover_stuck_delegation_ix(
        program_id,
        game_id,
        &white,
        &black,
        &authority.pubkey(),
    );
    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::RecoverStuckDelegation);
    let authority_bytes = authority.to_bytes();
    let result = tokio::task::spawn_blocking(move || {
        let kp = solana_sdk::signature::Keypair::try_from(authority_bytes.as_slice())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let rpc =
            solana::make_rpc(&rpc_url).map_err(|error| std::io::Error::other(error.to_string()))?;
        solana::sign_and_submit(&rpc, &kp, &[ix])
    })
    .await;

    match result {
        Ok(Ok(sig)) => {
            info!(
                "[settlement] game {} stuck-delegation escrow auto-recovered, sig {}",
                game_id, sig
            );
            worker_metrics::STUCK_DELEGATION_AUTO_RECOVERED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(Err(e)) => {
            warn!(
                "[settlement] game {}: recover_stuck_delegation auto-call failed ({}) — \
                 falling back to manual admin route",
                game_id, e
            );
            worker_metrics::FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Err(e) => {
            warn!(
                "[settlement] game {}: recover_stuck_delegation join error: {} — falling back \
                 to manual admin route",
                game_id, e
            );
            worker_metrics::FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Submits `finalize_game` and runs the off-chain bookkeeping once the Game PDA
/// is settled. A confirmation timeout is not treated as a failure: when the PDA
/// is gone afterwards the game was settled (by this transaction or by any other
/// caller of the permissionless instruction) and the bookkeeping still runs.
async fn finalize_on_chain(
    state: &Arc<AppState>,
    game_id: u64,
    session_kp: &solana_sdk::signature::Keypair,
    snap: &GameSnapshot,
) -> Result<(), String> {
    let program_id =
        Pubkey::from_str(&state.config.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    // Pass recorded game.fee_payer for rent reimbursement; session_kp still signs the transaction.
    let ix = solana::finalize_game_ix(
        &program_id,
        game_id,
        &snap.white,
        &snap.black,
        None,
        &snap.fee_payer,
    );

    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::FinalizeGame);
    let kp_bytes = session_kp.to_bytes();
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let submit_started = std::time::Instant::now();
    state.metrics.record_transaction_submitted("solana");
    let (signature, result, settled_anyway) = tokio::task::spawn_blocking(move || {
        let kp = solana_sdk::signature::Keypair::try_from(kp_bytes.as_slice())
            .map_err(|e| format!("bad keypair: {e}"))?;
        let rpc = solana::make_rpc(&rpc_url).map_err(|error| error.to_string())?;
        let (signature, result) = solana::sign_and_submit_tracked(&rpc, &kp, &[ix]);
        let result = result.map_err(|e| format!("finalize: {e}"));
        // Only a confirmed-missing account proves settlement; an RPC error does not.
        let settled_anyway = result.is_err()
            && matches!(rpc.get_account_with_commitment(&game_pda, rpc.commitment()),
                Ok(response) if response.value.is_none());
        let landed = match (&result, signature) {
            (Ok(_), _) => true,
            (Err(_), Some(sig)) if settled_anyway => {
                solana::signature_landed(&rpc, &sig).unwrap_or(false)
            }
            _ => false,
        };
        Ok::<_, String>((
            landed.then_some(signature).flatten(),
            result,
            settled_anyway,
        ))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    match result {
        Ok(_) => state
            .metrics
            .record_transaction_confirmed("solana", submit_started.elapsed().as_millis() as f64),
        Err(e) => {
            let category = solana::classify_error_str(&e).to_string();
            state.metrics.record_transaction_failed("solana", &category);
            if !settled_anyway {
                return Err(e);
            }
            warn!(
                "[settlement] game {}: finalize reported an error ({}) but the Game PDA is closed; \
                 recording the settlement (our signature landed: {})",
                game_id,
                e,
                signature.is_some()
            );
        }
    }

    worker_metrics::SETTLEMENT_FINALIZED_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let sig = signature.map(|s| s.to_string());
    info!(
        "[FINALIZED] game {} settled — status={} winner={:?}, payout sig {}",
        game_id,
        snap.status,
        snap.winner,
        sig.as_deref().unwrap_or("unknown")
    );
    if snap.country_fee > 0 {
        let treasury_vault =
            Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &program_id).0;
        info!(
            "[TREASURY] game {} paid {} lamports platform fee to treasury_vault {}",
            game_id, snap.country_fee, treasury_vault
        );
    }
    record_settled_game(state, game_id, snap, sig.as_deref()).await;
    Ok(())
}

/// The off-chain effects of a settled game: the SQLite record, PGN, rating
/// cache, anti-cheat analysis, the tournament result and session retirement.
/// Every value comes from `snap`, the on-chain Game read before settlement,
/// never from a client request. Callers run it once per settled game; each
/// step tolerates a repeat (upsert, `INSERT OR IGNORE`, idempotent orchestrator).
pub async fn record_settled_game(
    state: &AppState,
    game_id: u64,
    snap: &GameSnapshot,
    settlement_sig: Option<&str>,
) {
    let winner_side = match snap.winner {
        Some(w) if w == snap.white => Some("white"),
        Some(_) => Some("black"),
        None => None, // draw or cancellation
    };
    let repo = GameRepository::new(state.store.pool());
    let white = snap.white.to_string();
    let black = snap.black.to_string();
    let white_username = repo.get_username(&white).await.ok();
    let black_username = repo.get_username(&black).await.ok();
    if let Err(e) = repo
        .complete_game(crate::db::repository::CompletedGame {
            game_id: &game_id.to_string(),
            player_white: Some(&white),
            player_black: Some(&black),
            white_username: white_username.as_deref(),
            black_username: black_username.as_deref(),
            winner: winner_side,
            final_fen: None,
            finalize_sig: settlement_sig.unwrap_or_default(),
            stake_amount: snap.wager_amount as f64 / 1e9,
        })
        .await
    {
        error!(
            "[settlement] DB completion failed for game {}: {}",
            game_id, e
        );
    }

    crate::signing::game_pgn::assemble_and_store_pgn(
        &repo,
        &state.elo_cache,
        crate::signing::game_pgn::PgnGame {
            game_id_str: &game_id.to_string(),
            white: &white,
            black: &black,
            white_username: white_username.as_deref(),
            black_username: black_username.as_deref(),
            winner: winner_side,
        },
    )
    .await;
    state.elo_cache.invalidate(&white);
    state.elo_cache.invalidate(&black);

    // Crash-and-settle must not skip analysis.
    enqueue_game_analysis(
        state,
        FinalizedGame {
            game_id,
            white: white.clone(),
            black: black.clone(),
            winner: winner_side.map(str::to_string),
            wager_lamports: snap.wager_amount,
            tournament_id: snap.tournament_id,
            base_time_seconds: snap.base_time_seconds.min(u32::MAX as u64) as u32,
            increment_seconds: snap.increment_seconds as u32,
        },
    )
    .await;

    // Tournament games, any format and draws included, go to the orchestrator,
    // which applies the format's rules and mirrors the result on-chain.
    if let (Some(tournament_id), Some(tx)) = (snap.tournament_id, &state.orchestrator_tx) {
        let outcome = match winner_side {
            Some("white") => GameOutcome::WhiteWin,
            Some("black") => GameOutcome::BlackWin,
            _ => GameOutcome::Draw,
        };
        if let Err(e) = tx
            .send(OrchestratorEvent::GameEnded {
                tournament_id,
                game_id,
                white: white.clone(),
                black: black.clone(),
                outcome,
            })
            .await
        {
            warn!(
                "[settlement] Failed to queue tournament result for game {}: {}",
                game_id, e
            );
        }
    }

    state.store.deactivate(game_id).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;
    // Not imported at module scope: only the tests assert on the winner tag,
    // so importing it above would warn as unused in non-test builds.
    use crate::signing::solana::game_account::RESULT_WINNER;

    fn escrow_with(lamports: u64) -> Fetched {
        Fetched::Found(solana_sdk::account::Account {
            lamports,
            ..Default::default()
        })
    }

    #[test]
    fn missing_game_with_empty_escrow_is_resolved_after_grace() {
        let now = 1_000_000;
        let old = Some(now - MISSING_GAME_GRACE_SECS - 1);
        assert_eq!(
            resolve_missing_game(&Fetched::Missing, old, now),
            MissingGameResolution::Resolved
        );
        assert_eq!(
            resolve_missing_game(&escrow_with(1), old, now),
            MissingGameResolution::Resolved
        );
        // Legacy rows have created_at = 0 and are treated as old.
        assert_eq!(
            resolve_missing_game(&Fetched::Missing, Some(0), now),
            MissingGameResolution::Resolved
        );
    }

    #[test]
    fn missing_game_with_held_escrow_is_never_retired() {
        let now = 1_000_000;
        let old = Some(now - MISSING_GAME_GRACE_SECS - 1);
        assert_eq!(
            resolve_missing_game(&escrow_with(10_000_000), old, now),
            MissingGameResolution::FundsHeld(10_000_000)
        );
    }

    #[test]
    fn rpc_failure_or_recent_session_keeps_missing_game_pending() {
        let now = 1_000_000;
        let old = Some(now - MISSING_GAME_GRACE_SECS - 1);
        assert_eq!(
            resolve_missing_game(&Fetched::Unknown, old, now),
            MissingGameResolution::Unknown
        );
        assert_eq!(
            resolve_missing_game(&Fetched::Missing, None, now),
            MissingGameResolution::Unknown
        );
        assert_eq!(
            resolve_missing_game(&Fetched::Missing, Some(now - 30), now),
            MissingGameResolution::Grace
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn build_game_data(
        white: Pubkey,
        black: Pubkey,
        status: u8,
        winner: Option<Pubkey>,
        result_tag: u8,
        wager_token: Option<Pubkey>,
        is_delegated: bool,
        tournament_id: Option<u64>,
        updated_at: i64,
    ) -> Vec<u8> {
        let mut d = vec![0u8; 8]; // discriminator
        d.extend_from_slice(&42u64.to_le_bytes()); // game_id
        d.extend_from_slice(white.as_ref());
        d.extend_from_slice(black.as_ref());
        d.push(status);
        d.extend_from_slice(&0i64.to_le_bytes()); // last_move_timestamp
        d.extend_from_slice(&0u64.to_le_bytes()); // fees_advanced
        d.extend_from_slice(Pubkey::new_unique().as_ref()); // fee_payer
        d.push(result_tag);
        if let Some(w) = winner {
            d.extend_from_slice(w.as_ref());
        }
        d.extend_from_slice(&[0u8; 68]); // board_state
        d.extend_from_slice(&10u16.to_le_bytes()); // move_count

        // Include halfmove_clock to keep the fixture aligned with the on-chain layout.
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes()); // turn (u16)
        d.extend_from_slice(&0i64.to_le_bytes()); // created_at
        d.extend_from_slice(&updated_at.to_le_bytes());
        d.extend_from_slice(&1_000u64.to_le_bytes()); // wager_amount
        match wager_token {
            Some(m) => {
                d.push(1);
                d.extend_from_slice(m.as_ref());
            }
            None => d.push(0),
        }
        d.push(0); // game_type
        d.push(1); // match_type
        d.extend_from_slice(&0u64.to_le_bytes()); // country_fee
        d.extend_from_slice(&300u64.to_le_bytes()); // base_time_seconds
        d.extend_from_slice(&2u16.to_le_bytes()); // increment_seconds
        d.push(254); // bump
        d.push(if is_delegated { 1 } else { 0 });
        match tournament_id {
            Some(tid) => {
                d.push(1);
                d.extend_from_slice(&tid.to_le_bytes());
            }
            None => d.push(0),
        }
        d.extend_from_slice(&7u64.to_le_bytes()); // nonce
        d.push(0); // draw_offered_by = None
        d
    }

    #[test]
    fn parses_finished_game_with_winner() -> anyhow::Result<()> {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let data = build_game_data(
            white,
            black,
            STATUS_FINISHED,
            Some(white),
            RESULT_WINNER,
            None,
            false,
            None,
            0,
        );
        let snap = parse_game_account(&data).context("should parse")?;
        assert_eq!(snap.white, white);
        assert_eq!(snap.black, black);
        assert_eq!(snap.status, STATUS_FINISHED);
        assert_eq!(snap.result_tag, RESULT_WINNER);
        assert_eq!(snap.winner, Some(white));
        assert_eq!(snap.wager_amount, 1_000);
        assert_eq!(snap.base_time_seconds, 300);
        assert_eq!(snap.increment_seconds, 2);
        assert_eq!(snap.tournament_id, None);
        assert!(!snap.is_delegated);
        Ok(())
    }

    #[test]
    fn parses_delegated_game_in_progress() -> anyhow::Result<()> {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        // Active game (status 2), no result, SPL wager token, delegated to ER,
        // part of tournament 99.
        let data = build_game_data(
            white,
            black,
            2,
            None,
            RESULT_NONE,
            Some(Pubkey::new_unique()),
            true,
            Some(99),
            0,
        );
        let snap = parse_game_account(&data).context("should parse")?;
        assert_eq!(snap.status, 2);
        assert_eq!(snap.result_tag, RESULT_NONE);
        assert_eq!(snap.winner, None);
        assert_eq!(snap.tournament_id, Some(99));
        assert!(snap.is_delegated);
        Ok(())
    }

    #[test]
    fn parses_updated_at_for_staleness_check() -> anyhow::Result<()> {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let long_ago = 1_700_000_000i64;
        let data = build_game_data(
            white,
            black,
            2,
            None,
            RESULT_NONE,
            None,
            true,
            None,
            long_ago,
        );
        let snap = parse_game_account(&data).context("should parse")?;
        assert_eq!(snap.updated_at, long_ago);

        let now = long_ago + STALE_DELEGATION_SECS + 1;
        assert!(now.saturating_sub(snap.updated_at) > STALE_DELEGATION_SECS);
        Ok(())
    }

    #[test]
    fn parses_active_undelegated_wagered_game() -> anyhow::Result<()> {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let long_ago = 1_700_000_000i64;
        let data = build_game_data(
            white,
            black,
            STATUS_ACTIVE,
            None,
            RESULT_NONE,
            None,
            false, // not delegated
            None,
            long_ago,
        );
        let snap = parse_game_account(&data).context("should parse")?;
        assert_eq!(snap.status, STATUS_ACTIVE);
        assert!(!snap.is_delegated);
        assert!(snap.wager_amount > 0, "test fixture must be wagered");

        let now = long_ago + STALE_UNDELEGATED_SECS + 1;
        assert!(now.saturating_sub(snap.updated_at) > STALE_UNDELEGATED_SECS);
        // And comfortably shorter than the delegated-staleness window — no
        // ER-liveness excuse applies to a game that was never delegated.
        const {
            assert!(STALE_UNDELEGATED_SECS < STALE_DELEGATION_SECS);
        }
        Ok(())
    }

    fn er_copy(status: u8, result_tag: u8, updated_at: i64) -> anyhow::Result<GameSnapshot> {
        Ok({
            let white = Pubkey::new_unique();
            let data = build_game_data(
                white,
                Pubkey::new_unique(),
                status,
                (result_tag == RESULT_WINNER).then_some(white),
                result_tag,
                None,
                true,
                None,
                updated_at,
            );
            parse_game_account(&data).context("fixture parses")?
        })
    }

    #[test]
    fn healthy_er_games_are_never_sent_to_the_escape_hatch() -> anyhow::Result<()> {
        // Fixture: 300s base, 2s increment, 10 plies -> a 310s clock for the mover.
        let now = 1_000_000;
        let live = er_copy(STATUS_ACTIVE, RESULT_NONE, now - 300)?;
        assert_eq!(delegated_action(Some(&live), now), DelegatedAction::Wait);

        let abandoned = er_copy(STATUS_ACTIVE, RESULT_NONE, now - 311)?;
        assert_eq!(
            delegated_action(Some(&abandoned), now),
            DelegatedAction::ClaimTimeout,
            "an abandoned game ends on time, not by a 50/50 wipe"
        );
        Ok(())
    }

    #[test]
    fn terminal_er_games_are_returned_for_settlement() -> anyhow::Result<()> {
        let now = 1_000_000;
        let won = er_copy(STATUS_FINISHED, RESULT_WINNER, now)?;
        assert_eq!(
            delegated_action(Some(&won), now),
            DelegatedAction::Undelegate
        );
        let cancelled = er_copy(STATUS_CANCELLED, RESULT_NONE, now)?;
        assert_eq!(
            delegated_action(Some(&cancelled), now),
            DelegatedAction::Undelegate
        );
        Ok(())
    }

    #[test]
    fn missing_or_returned_er_copies_need_a_health_check() -> anyhow::Result<()> {
        let now = 1_000_000;
        assert_eq!(delegated_action(None, now), DelegatedAction::CheckErHealth);
        let mut in_flight = er_copy(STATUS_FINISHED, RESULT_WINNER, now)?;
        in_flight.is_delegated = false;
        assert_eq!(
            delegated_action(Some(&in_flight), now),
            DelegatedAction::CheckErHealth
        );
        Ok(())
    }

    #[test]
    fn rejects_truncated_account() {
        assert!(parse_game_account(&[0u8; 40]).is_none());
    }

    #[test]
    fn parses_undelegation_request_expiry() {
        let mut data = vec![0u8; 8]; // discriminator
        data.extend_from_slice(Pubkey::new_unique().as_ref()); // delegated_account
        data.extend_from_slice(&123_456_789u64.to_le_bytes()); // expires_at_slot
        assert_eq!(parse_undelegation_request_expiry(&data), Some(123_456_789));
    }

    #[test]
    fn rejects_truncated_undelegation_request() {
        assert!(parse_undelegation_request_expiry(&[0u8; 20]).is_none());
    }
}
