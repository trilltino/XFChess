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

/// GameStatus discriminants (borsh enum tags, see programs/.../state/game.rs).
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
async fn fetch_accounts_batched(rpc_url: String, pdas: Vec<Pubkey>) -> Vec<Fetched> {
    tokio::task::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        let rpc = solana::make_rpc(&rpc_url);
        let mut out = Vec::with_capacity(pdas.len());
        for chunk in pdas.chunks(RPC_BATCH_SIZE) {
            worker_metrics::SETTLEMENT_RPC_CALLS_TOTAL.fetch_add(1, Ordering::Relaxed);
            match rpc.get_multiple_accounts(chunk) {
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
            let scanned = match run_tick(&state).await {
                Ok(n) => n,
                Err(e) => {
                    warn!("[settlement] tick failed: {e}");
                    0
                }
            };
            use std::sync::atomic::Ordering;
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

    let fetched = fetch_accounts_batched(state.config.solana_rpc_url.clone(), pdas.clone()).await;
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
                        if now.saturating_sub(snap.updated_at) > STALE_DELEGATION_SECS {
                            stale_delegated += 1;
                            warn!(
                                "[settlement] game {} has been delegated with no on-chain \
                                 activity for over {}m — possible stuck ER delegation",
                                game_id,
                                STALE_DELEGATION_SECS / 60
                            );
                        }
                        delegated.push(i)
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
        let er_fetched = fetch_accounts_batched(state.config.er_rpc_url.clone(), er_pdas).await;
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
    let er_url = state.config.magic_router_rpc_url.clone();
    let sig = tokio::task::spawn_blocking(move || {
        let rpc = solana::make_rpc(&er_url);
        solana::sign_and_submit_er(&rpc, &session_kp, &[ix])
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
    .map_err(|e| format!("undelegate: {e}"))?;
    worker_metrics::SETTLEMENT_UNDELEGATED_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    info!(
        "[settlement] game {} finished on ER — undelegated, sig {}",
        game_id, sig
    );
    Ok(())
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

    let rpc_url = state.config.solana_rpc_url.clone();
    let kp_bytes = session_kp.to_bytes();
    let sig = tokio::task::spawn_blocking(move || {
        let kp = solana_sdk::signature::Keypair::from_bytes(&kp_bytes)
            .map_err(|e| format!("bad keypair: {e}"))?;
        let rpc = solana::make_rpc(&rpc_url);
        solana::sign_and_submit(&rpc, &kp, &[ix]).map_err(|e| format!("finalize: {e}"))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

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

    #[test]
    fn rejects_truncated_account() {
        assert!(parse_game_account(&[0u8; 40]).is_none());
    }
}
