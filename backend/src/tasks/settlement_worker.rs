use crate::db::repository::GameRepository;
use crate::signing::anticheat_enqueue::{enqueue_game_analysis, FinalizedGame};
use crate::signing::solana::{self, GAME_SEED};
use crate::signing::storage::tournament::TournamentFormat;
use crate::signing::swiss::orchestrator::OrchestratorEvent;
use crate::signing::AppState;
use crate::telemetry::worker_metrics;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

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
        let rpc = solana::make_rpc(&rpc_url);
        let mut out = Vec::with_capacity(pdas.len());
        for chunk in pdas.chunks(RPC_BATCH_SIZE) {
            worker_metrics::SETTLEMENT_RPC_CALLS_TOTAL.fetch_add(1, Ordering::Relaxed);
            let started = std::time::Instant::now();
            let result = rpc.get_multiple_accounts(chunk);
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
            let scanned = match run_tick(&state).await {
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
    let game_ids = state.store.list_active_game_ids().await;
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
    for (game_id, fetched) in game_ids.into_iter().zip(fetched) {
        let terminal = match fetched {
            Fetched::Missing => true,
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

    Ok(deactivated)
}

async fn run_tick(state: &Arc<AppState>) -> Result<u64, String> {
    let game_ids = state.store.list_active_game_ids().await;
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

    // Indices of games whose devnet copy says they're delegated to the ER.
    let mut delegated: Vec<usize> = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut stale_delegated: u64 = 0;

    for (i, f) in fetched.iter().enumerate() {
        let game_id = game_ids[i];
        match f {
            Fetched::Unknown => {}
            Fetched::Missing => {
                // Account closed — finalize already ran and reclaimed the rent.
                state.store.deactivate(game_id).await;
            }
            Fetched::Found(account) => {
                let Some(snap) = parse_game_account(&account.data) else {
                    warn!("[settlement] game {}: unparseable game account", game_id);
                    continue;
                };
                match snap.status {
                    STATUS_SETTLED | STATUS_EXPIRED => {
                        state.store.deactivate(game_id).await;
                    }
                    STATUS_CANCELLED if !snap.is_delegated => {
                        match state.store.get(game_id).await {
                            Some(entry) => {
                                if let Err(e) =
                                    finalize_on_chain(state, game_id, &entry.keypair(), &snap).await
                                {
                                    warn!("[settlement] game {}: {}", game_id, e);
                                }
                            }
                            None => warn!("[settlement] game {}: session disappeared", game_id),
                        }
                    }
                    STATUS_FINISHED if snap.result_tag != RESULT_NONE && !snap.is_delegated => {
                        match state.store.get(game_id).await {
                            Some(entry) => {
                                if let Err(e) =
                                    finalize_on_chain(state, game_id, &entry.keypair(), &snap).await
                                {
                                    warn!("[settlement] game {}: {}", game_id, e);
                                }
                            }
                            None => warn!("[settlement] game {}: session disappeared", game_id),
                        }
                    }
                    _ if snap.is_delegated => {
                        let stale_for = now.saturating_sub(snap.updated_at);
                        if stale_for > STALE_DELEGATION_SECS {
                            stale_delegated += 1;
                            warn!(
                                "[settlement] game {} has been delegated with no on-chain \
                                 activity for over {}m — possible stuck ER delegation",
                                game_id,
                                STALE_DELEGATION_SECS / 60
                            );
                            // Fire the (idempotent, safe) forced-undelegation
                            // request once, shortly after crossing the
                            // staleness threshold — not on every tick
                            // thereafter. See MAGICBLOCK.md's "Failure Mode:
                            // ER Unavailability" section.
                            if stale_for
                                < STALE_DELEGATION_SECS + SETTLEMENT_TICK.as_secs() as i64 * 2
                            {
                                request_force_undelegate_for_stale_game(
                                    state,
                                    game_id,
                                    &program_id,
                                )
                                .await;
                            }
                            // Once the ~60min request window has elapsed,
                            // complete the recovery without the ER at all,
                            // and release the escrow in the same breath —
                            // `snap.white`/`snap.black` are read here, before
                            // the wipe, since the escrow-release step can't
                            // recover them from the (by then empty) account.
                            force_undelegate_if_request_expired(
                                state,
                                game_id,
                                &program_id,
                                snap.white,
                                snap.black,
                            )
                            .await;
                        }
                        delegated.push(i)
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

    // The devnet copy is frozen while delegated; check the live ER copies and
    // pull finished games back to devnet so finalize can run next tick.
    if !delegated.is_empty() {
        let er_pdas: Vec<Pubkey> = delegated.iter().map(|&i| pdas[i]).collect();
        let er_fetched = fetch_accounts_batched(
            state.config.er_rpc_url.clone(),
            er_pdas,
            state.metrics.clone(),
        )
        .await;
        for (j, f) in er_fetched.iter().enumerate() {
            let game_id = game_ids[delegated[j]];
            if let Fetched::Found(acc) = f {
                if let Some(er_snap) = parse_game_account(&acc.data) {
                    if er_snap.status == STATUS_FINISHED && er_snap.result_tag != RESULT_NONE {
                        if let Err(e) = undelegate_from_er(state, game_id, &program_id).await {
                            warn!("[settlement] game {}: {}", game_id, e);
                        }
                    }
                }
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
) -> Result<(), String> {
    let entry = state
        .store
        .get(game_id)
        .await
        .ok_or("session disappeared")?;
    let session_kp = entry.keypair();
    let session_pk = session_kp.pubkey();
    let ix = solana::undelegate_game_ix(program_id, &session_pk, game_id)
        .map_err(|e| format!("build undelegate: {e}"))?;
    let er_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::UndelegateGame);
    let submit_started = std::time::Instant::now();
    state.metrics.record_transaction_submitted("er");
    let sig_result = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&er_url);
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

    // Cancel the time-check crank as a best-effort follow-up so a finished
    // game doesn't leave a dangling scheduled task on the ER. Never let this
    // affect the undelegate result already recorded above.
    let cancel_kp = entry.keypair();
    let cancel_pk = cancel_kp.pubkey();
    let cancel_program_id = *program_id;
    match solana::cancel_time_check_ix(&cancel_program_id, &cancel_pk, game_id) {
        Ok(cancel_ix) => {
            let er_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::CancelTimeCheck);
            let cancel_result = tokio::task::spawn_blocking(move || {
                let rpc = solana::make_rpc(&er_url);
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
    let Some(entry) = state.store.get(game_id).await else {
        warn!(
            "[settlement] game {}: session disappeared before redelegate attempt",
            game_id
        );
        return;
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
            .expect("valid keypair bytes");
        let session_kp = solana_sdk::signature::Keypair::try_from(session_bytes.as_slice())
            .expect("valid keypair bytes");
        let rpc = solana::make_rpc(&rpc_url);
        let blockhash = rpc.get_latest_blockhash()?;
        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer_kp.pubkey()),
            &[&payer_kp, &session_kp],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx)
    })
    .await;

    match result {
        Ok(Ok(sig)) => {
            info!("[settlement] game {} redelegated, sig {}", game_id, sig);
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
                .expect("valid keypair bytes");
            let rpc = solana::make_rpc(&rpc_url);
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

async fn force_undelegate_if_request_expired(
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
    let (request_account, current_slot) = match tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&rpc_url);
        (rpc.get_account(&request_pda).ok(), rpc.get_slot().ok())
    })
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            warn!(
                "[settlement] game {}: request-expiry check join error: {}",
                game_id, e
            );
            return;
        }
    };
    let (Some(account), Some(current_slot)) = (request_account, current_slot) else {
        return; // no request yet, or RPC hiccup — retried next tick
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
                .expect("valid keypair bytes");
            let rpc = solana::make_rpc(&rpc_url);
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
            .expect("valid keypair bytes");
        let rpc = solana::make_rpc(&rpc_url);
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

async fn finalize_on_chain(
    state: &Arc<AppState>,
    game_id: u64,
    session_kp: &solana_sdk::signature::Keypair,
    snap: &GameSnapshot,
) -> Result<(), String> {
    let program_id =
        Pubkey::from_str(&state.config.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    let winner_side = match snap.winner {
        Some(w) if w == snap.white => Some("white"),
        Some(_) => Some("black"),
        None => None, // draw
    };
    // finalize now requires the passed fee_payer to equal the recorded
    // game.fee_payer (rent + reimbursement go there); the tx is still signed by
    // session_kp, but fee_payer is a non-signer account.
    let ix = solana::finalize_game_ix(
        &program_id,
        game_id,
        &snap.white,
        &snap.black,
        winner_side,
        &snap.fee_payer,
    );

    let rpc_url = solana::rpc_url_for(&state.config, solana::RoutedInstr::FinalizeGame);
    let kp_bytes = session_kp.to_bytes();
    let submit_started = std::time::Instant::now();
    state.metrics.record_transaction_submitted("solana");
    let sig_result = tokio::task::spawn_blocking(move || {
        let kp = solana_sdk::signature::Keypair::try_from(kp_bytes.as_slice())
            .map_err(|e| format!("bad keypair: {e}"))?;
        let rpc = solana::make_rpc(&rpc_url);
        solana::sign_and_submit(&rpc, &kp, &[ix]).map_err(|e| format!("finalize: {e}"))
    })
    .await
    .map_err(|e| format!("join error: {e}"))?;
    let sig = match sig_result {
        Ok(sig) => {
            state.metrics.record_transaction_confirmed(
                "solana",
                submit_started.elapsed().as_millis() as f64,
            );
            sig
        }
        Err(e) => {
            let category = solana::classify_error_str(&e).to_string();
            state.metrics.record_transaction_failed("solana", &category);
            return Err(e);
        }
    };

    worker_metrics::SETTLEMENT_FINALIZED_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    info!(
        "[FINALIZED] game {} auto-settled on devnet — winner={:?}, payout sig {} — inspect: https://solscan.io/tx/{}?cluster=devnet",
        game_id, winner_side, sig, sig
    );
    if snap.country_fee > 0 {
        let treasury_vault =
            Pubkey::find_program_address(&[solana::TREASURY_VAULT_SEED], &program_id).0;
        info!(
            "[TREASURY] game {} paid {} lamports platform fee to treasury_vault {}",
            game_id, snap.country_fee, treasury_vault
        );
    }

    // Mirror the result into the SQLite game record.
    let repo = GameRepository::new(state.store.pool());
    let white = snap.white.to_string();
    let black = snap.black.to_string();
    let white_username = repo.get_username(&white).await.ok();
    let black_username = repo.get_username(&black).await.ok();
    if let Err(e) = repo
        .complete_game(
            &game_id.to_string(),
            Some(&white),
            Some(&black),
            white_username.as_deref(),
            black_username.as_deref(),
            winner_side,
            None,
            &sig.to_string(),
            snap.wager_amount as f64 / 1e9,
        )
        .await
    {
        error!(
            "[settlement] DB completion failed for game {}: {}",
            game_id, e
        );
    }

    // Same PGN assembly as the HTTP finalize route (routes::main::finalize_game)
    // — most games settle through this auto-worker, not the manual route, so
    // this is the path that actually needs to produce a tagged PGN.
    crate::signing::game_pgn::assemble_and_store_pgn(
        &repo,
        &state.elo_cache,
        &game_id.to_string(),
        &white,
        &black,
        white_username.as_deref(),
        black_username.as_deref(),
        winner_side,
    )
    .await;
    state.elo_cache.invalidate(&white);
    state.elo_cache.invalidate(&black);

    // Same anti-cheat path as the HTTP finalize route — auto-settled games
    // must not skip analysis (crash-and-settle is the cheater's exit).
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

    if let Some(tournament_id) = snap.tournament_id {
        let is_swiss = state
            .tournament_store
            .get(tournament_id)
            .await
            .is_some_and(|t| matches!(t.format, TournamentFormat::Swiss { .. }));
        if is_swiss {
            if let Some(tx) = &state.orchestrator_tx {
                let result = match winner_side {
                    Some("white") => swiss_pairing::MatchResult::WhiteWin,
                    Some("black") => swiss_pairing::MatchResult::BlackWin,
                    _ => swiss_pairing::MatchResult::Draw,
                };
                if let Err(e) = tx
                    .send(OrchestratorEvent::GameEnded {
                        tournament_id,
                        game_id,
                        result,
                    })
                    .await
                {
                    warn!(
                        "[settlement] Failed to queue Swiss result for game {}: {}",
                        game_id, e
                    );
                }
            }
        } else if let Some(tx) = &state.tournament_trigger {
            if let Some((winner, loser)) = match winner_side {
                Some("white") => Some((white.clone(), black.clone())),
                Some("black") => Some((black.clone(), white.clone())),
                _ => None,
            } {
                if let Err(e) = tx
                    .send(crate::signing::TournamentTrigger::GameSettled {
                        tournament_id,
                        game_id,
                        winner,
                        loser,
                    })
                    .await
                {
                    warn!(
                        "[settlement] Failed to queue elimination result for game {}: {}",
                        game_id, e
                    );
                }
            }
        }
    }

    state.store.deactivate(game_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Not imported at module scope: only the tests assert on the winner tag,
    // so importing it above would warn as unused in non-test builds.
    use crate::signing::solana::game_account::RESULT_WINNER;

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

        // halfmove_clock — omitted here originally, which shifted every
        // field below it by two bytes and made the fixture disagree with the
        // real on-chain layout (see `signing::solana::game_account`).
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes()); // turn (u16)
        d.extend_from_slice(&0i64.to_le_bytes()); // created_at
        d.extend_from_slice(&updated_at.to_le_bytes()); // updated_at
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
    fn parses_finished_game_with_winner() {
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
        let snap = parse_game_account(&data).expect("should parse");
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
    }

    #[test]
    fn parses_delegated_game_in_progress() {
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
        let snap = parse_game_account(&data).expect("should parse");
        assert_eq!(snap.status, 2);
        assert_eq!(snap.result_tag, RESULT_NONE);
        assert_eq!(snap.winner, None);
        assert_eq!(snap.tournament_id, Some(99));
        assert!(snap.is_delegated);
    }

    #[test]
    fn parses_updated_at_for_staleness_check() {
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
        let snap = parse_game_account(&data).expect("should parse");
        assert_eq!(snap.updated_at, long_ago);

        let now = long_ago + STALE_DELEGATION_SECS + 1;
        assert!(now.saturating_sub(snap.updated_at) > STALE_DELEGATION_SECS);
    }

    #[test]
    fn parses_active_undelegated_wagered_game() {
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
        let snap = parse_game_account(&data).expect("should parse");
        assert_eq!(snap.status, STATUS_ACTIVE);
        assert!(!snap.is_delegated);
        assert!(snap.wager_amount > 0, "test fixture must be wagered");

        let now = long_ago + STALE_UNDELEGATED_SECS + 1;
        assert!(now.saturating_sub(snap.updated_at) > STALE_UNDELEGATED_SECS);
        // And comfortably shorter than the delegated-staleness window — no
        // ER-liveness excuse applies to a game that was never delegated.
        assert!(STALE_UNDELEGATED_SECS < STALE_DELEGATION_SECS);
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
