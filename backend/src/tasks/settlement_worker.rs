//! Auto-settlement worker — makes wager payout fully automatic.
//!
//! Every tick it scans active game sessions, reads each Game PDA from chain,
//! and drives finished games to settlement without any client action:
//!
//! * result committed on devnet  → submit `finalize_game` (pays the escrow out)
//! * game still delegated to ER  → if the ER copy shows a finished game,
//!   submit `undelegate_game` so finalize can run on the next tick
//! * game settled / closed       → mark the session inactive
//!
//! This is the safety net behind the `/game/finalize` HTTP endpoint: if the
//! client crashes or disconnects after the result was committed on-chain, the
//! winner is still paid.

use crate::db::repository::GameRepository;
use crate::signing::anticheat_enqueue::{enqueue_game_analysis, FinalizedGame};
use crate::signing::solana::{self, GAME_SEED};
use crate::signing::AppState;
use crate::telemetry::worker_metrics;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// How often the worker scans active sessions.
const SETTLEMENT_TICK: Duration = Duration::from_secs(30);

/// A delegated game with no on-chain activity for longer than this is
/// flagged as possibly stuck (see `SETTLEMENT_STALE_DELEGATED_GAUGE`).
/// Generous on purpose — normal games settle in minutes, so this is chosen
/// to comfortably clear any real game's `base_time_seconds` + increment
/// budget while still catching a genuinely stalled ER delegation within a
/// reasonable ops window.
const STALE_DELEGATION_SECS: i64 = 20 * 60;

/// An active, wagered game that's still not delegated to the ER after this
/// long almost certainly means the client-side delegation attempt (see
/// `src/multiplayer/rollup/bridge.rs::handle_game_start_delegation`) failed
/// or never ran — a normal create/join → delegate handshake takes seconds,
/// not minutes. Shorter than `STALE_DELEGATION_SECS`: there's no ER-liveness
/// excuse for this case, since nothing has been sent to the ER yet.
const STALE_UNDELEGATED_SECS: i64 = 5 * 60;

/// GameStatus discriminants (borsh enum tags, see programs/.../state/game.rs).
const STATUS_ACTIVE: u8 = 2;
const STATUS_FINISHED: u8 = 5;
const STATUS_SETTLED: u8 = 6;
const STATUS_EXPIRED: u8 = 7;
const STATUS_CANCELLED: u8 = 8;

/// GameResult borsh tags.
const RESULT_NONE: u8 = 0;
const RESULT_WINNER: u8 = 1;

/// The fields of an on-chain `Game` account the worker needs.
struct GameSnapshot {
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    status: u8,
    result_tag: u8,
    winner: Option<Pubkey>,
    wager_amount: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
    is_delegated: bool,
    tournament_id: Option<u64>,
    /// Unix timestamp of the game's last on-chain update (last move, or last
    /// commit while delegated). Used only for the stale-delegation gauge.
    updated_at: i64,
}

/// Walks the borsh layout of the Game account (8-byte Anchor discriminator,
/// then fields in declaration order). Enum/Option fields are compact-encoded,
/// so everything after `result` sits at a variable offset.
fn parse_game_account(data: &[u8]) -> Option<GameSnapshot> {
    let mut o = 8usize; // discriminator
    o += 8; // game_id
    let white = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let black = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32;
    let status = *data.get(o)?;
    o += 1;
    o += 8; // last_move_timestamp
    o += 8; // fees_advanced
    let fee_payer = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
    o += 32; // fee_payer
    let result_tag = *data.get(o)?;
    o += 1;
    let winner = if result_tag == RESULT_WINNER {
        let w = Pubkey::try_from(data.get(o..o + 32)?).ok()?;
        o += 32;
        Some(w)
    } else {
        None
    };
    o += 68; // board_state
    o += 2; // move_count
    o += 2; // turn (u16)
    o += 8; // created_at
    let updated_at = i64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);
    o += 8; // updated_at
    let wager_amount = u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);
    o += 8;
    let wager_token_tag = *data.get(o)?; // Option<Pubkey>
    o += 1;
    if wager_token_tag == 1 {
        o += 32;
    }
    o += 1; // game_type
    o += 1; // match_type
    o += 8; // country_fee
    let base_time_seconds = u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?);
    o += 8;
    let increment_seconds = u16::from_le_bytes(data.get(o..o + 2)?.try_into().ok()?);
    o += 2;
    o += 1; // bump
    let is_delegated = *data.get(o)? != 0;
    o += 1;
    let tournament_id = match *data.get(o)? {
        // Option<u64>
        1 => {
            o += 1;
            Some(u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?))
        }
        _ => None,
    };

    Some(GameSnapshot {
        white,
        black,
        fee_payer,
        status,
        result_tag,
        winner,
        wager_amount,
        base_time_seconds,
        increment_seconds,
        is_delegated,
        tournament_id,
        updated_at,
    })
}

/// `getMultipleAccounts` accepts at most this many pubkeys per call.
const RPC_BATCH_SIZE: usize = 100;

/// Result of one slot in a batched account fetch.
enum Fetched {
    /// The RPC chunk failed — state unknown, retry next tick.
    Unknown,
    /// Account does not exist (closed: finalize already reclaimed the rent).
    Missing,
    Found(solana_sdk::account::Account),
}

/// Fetches many accounts in `RPC_BATCH_SIZE` chunks on one blocking thread.
/// A failed chunk degrades to `Unknown` for its games instead of failing the
/// whole tick. The returned vec is aligned with `pdas`.
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

/// Spawns the background settlement loop.
pub fn spawn_settlement_worker(state: Arc<AppState>) {
    tokio::spawn(async move {
        info!(
            "[settlement] Auto-settlement worker started ({}s interval)",
            SETTLEMENT_TICK.as_secs()
        );
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

/// One scan pass: batch-fetch every active game's devnet account, settle what
/// can be settled, then batch-check the ER copies of delegated games.
/// Returns the number of games scanned.
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
                    STATUS_SETTLED | STATUS_EXPIRED | STATUS_CANCELLED => {
                        state.store.deactivate(game_id).await;
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

/// Submits `undelegate_game` on the ER so the finished game returns to devnet.
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

/// Attempts to redelegate a game whose devnet copy is active, wagered, and
/// still not delegated well past the time a normal create/join → delegate
/// handshake should take. Reuses the exact instruction and signer pair the
/// `/game/delegate` HTTP handler uses (`delegate_game` in
/// `signing/routes/main.rs`) — the session key this worker already holds
/// for the game (`entry.keypair()`) is the same `fee_payer` the on-chain
/// handler checks against `game.fee_payer`. On success, also (re)registers
/// the time-check crank exactly like that handler does, since a game that
/// skipped delegation also skipped crank scheduling.
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
            let base_rpc = solana::make_rpc(&state.config.solana_rpc_url);
            crate::signing::routes::main::schedule_time_check_crank(
                state,
                &base_rpc,
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

/// Attempts `request_force_undelegate` for a game whose delegation looks
/// stuck — starts the delegation program's ~60min countdown, after which
/// `force_undelegate_after_timeout` can recover it without the ER at all
/// (see `governance_ix::recover_stuck_delegation` for the escrow release
/// that follows). Runs on base RPC directly, **not** the Magic Router: the
/// whole point is to act without depending on the (possibly dead) ER, and
/// Magic Router would otherwise see `game` owned by the delegation program
/// and try to route this to the ER itself. Calling this again once a
/// request already exists is a harmless on-chain no-op, so re-firing on a
/// later tick (e.g. after a restart) is safe.
///
/// Tries each fee-payer-pool key in turn since the worker doesn't track
/// which one funded this specific game's original `delegate_game` call — a
/// wrong key just fails the CPI's `delegation_metadata.rent_payer` check
/// harmlessly, so this is safe to attempt speculatively.
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

/// `UndelegationRequest`'s on-chain layout (see `dlp_api::state::undelegation_request`):
/// 8-byte discriminator, 32-byte `delegated_account`, 8-byte `expires_at_slot` (LE u64).
fn parse_undelegation_request_expiry(data: &[u8]) -> Option<u64> {
    let o = 8 + 32;
    Some(u64::from_le_bytes(data.get(o..o + 8)?.try_into().ok()?))
}

/// If a `request_force_undelegate` was issued for this game and its ~60min
/// window has elapsed, completes the recovery with `force_undelegate_after_timeout`
/// — no ER involvement needed — and then immediately releases the escrow via
/// `recover_stuck_delegation` using `white`/`black` captured from the last
/// on-chain read before the wipe. No-ops quietly if no request exists yet,
/// or if it hasn't expired yet.
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

/// Completes the ER-unavailability escape hatch by releasing the wager
/// escrow (`governance_ix::recover_stuck_delegation`) right after
/// `force_undelegate_after_timeout` wipes the `Game` PDA — see
/// `programs/xfchess-game/src/governance_ix/recover_stuck_delegation.rs` for
/// why `white`/`black` must be supplied rather than read back on-chain.
///
/// Uses the same `DISPUTE_AUTHORITY_KEYPAIR` env var and instruction as the
/// manual `POST /admin/dispute/recover_stuck_delegation` route — this just
/// removes the human from the common-case path. If the env var isn't set,
/// or the on-chain call itself fails (e.g. escrow already drained by a
/// concurrent manual call), falls back to the pre-automation behavior:
/// logs it and increments `FORCE_UNDELEGATED_AWAITING_RECOVERY_TOTAL` so the
/// admin route remains the fallback.
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
    let authority = crate::signing::load_keypair_from_env_value(&authority_key);

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

/// Submits `finalize_game`, which pays out the wager escrow on-chain, then
/// completes the DB record and retires the session.
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
        "[settlement] game {} auto-finalized, winner={:?}, sig {}",
        game_id, winner_side, sig
    );

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

    // Same anti-cheat path as the HTTP finalize route — auto-settled games
    // must not skip analysis (crash-and-settle is the cheater's exit).
    enqueue_game_analysis(
        state,
        FinalizedGame {
            game_id,
            white,
            black,
            winner: winner_side.map(str::to_string),
            wager_lamports: snap.wager_amount,
            tournament_id: snap.tournament_id,
            base_time_seconds: snap.base_time_seconds.min(u32::MAX as u64) as u32,
            increment_seconds: snap.increment_seconds as u32,
        },
    )
    .await;

    state.store.deactivate(game_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes a Game account exactly as Anchor/borsh lays it out.
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

    /// The stale-delegation gauge (Phase 5 of the persistency roadmap) is
    /// only as good as `updated_at` actually round-tripping through the
    /// borsh layout — this pins that down so a future field reorder in
    /// `state/game.rs` is caught here instead of silently breaking the
    /// on-call signal.
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

    /// Pins the condition `run_tick` uses to route an active, wagered,
    /// never-delegated game into `redelegate_stale_game` — a client-side
    /// delegation attempt that failed or never ran (Fix D). Without this
    /// branch such a game previously fell into the silent `_ => {}`
    /// catch-all with no watchdog at all.
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

    /// Pins the `UndelegationRequest` byte layout (8-byte discriminator +
    /// 32-byte `delegated_account` + 8-byte `expires_at_slot`) that
    /// `force_undelegate_if_request_expired` relies on to know when the
    /// ~60min forced-recovery window has elapsed.
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
