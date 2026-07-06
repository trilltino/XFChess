//! Shared anti-cheat enqueue path for finalized games.
//!
//! Both settlement routes — the `/game/finalize` HTTP handler and the
//! auto-settlement worker — call [`enqueue_game_analysis`] so that every
//! settled game enters the analysis pipeline with the same context
//! (tournament round, real wager) regardless of how it was finalized.

use crate::signing::AppState;
use crate::telemetry::worker_metrics;
use std::sync::atomic::Ordering;
use tracing::{error, info, warn};
use xfchess_anticheat::ingest::{build_game_record, GameMeta, MoveRow};

/// Games shorter than this are too thin to score meaningfully.
const MIN_PLIES: usize = 10;

/// Everything the enqueue path needs to know about a settled game.
pub struct FinalizedGame {
    pub game_id: u64,
    pub white: String,
    pub black: String,
    /// "white" | "black" | None (draw).
    pub winner: Option<String>,
    pub wager_lamports: u64,
    /// From the on-chain Game account when available; when `None` the
    /// tournament store is scanned for a match holding this game_id.
    pub tournament_id: Option<u64>,
    /// 0 = unknown; a default is substituted.
    pub base_time_seconds: u32,
    pub increment_seconds: u32,
}

/// Builds a `GameRecord` from the stored moves and queues it for Stockfish
/// analysis. Also writes a durable row to the `anticheat_queue` table so a
/// full in-memory queue (or a crash) loses nothing — the row is deleted by
/// the worker on success.
pub async fn enqueue_game_analysis(state: &AppState, game: FinalizedGame) {
    let Some(queue) = state.anticheat_queue.clone() else {
        return;
    };
    let pool = state.store.pool();
    let game_id_str = game.game_id.to_string();

    type RawMove = (
        i64,
        String,
        Option<String>,
        String,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
    );
    let raw_moves: Vec<RawMove> = sqlx::query_as(
        "SELECT m.move_number, m.move_uci, m.fen_after, m.player, m.timestamp,
                COALESCE(t.blurred, 0), t.think_ms, t.reported_at
         FROM moves m
         LEFT JOIN move_telemetry t
                ON t.game_id = m.game_id AND t.move_number = m.move_number
         WHERE m.game_id = ? ORDER BY m.move_number ASC",
    )
    .bind(&game_id_str)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if raw_moves.len() < MIN_PLIES {
        return;
    }

    let mut rows: Vec<MoveRow> = raw_moves
        .iter()
        .map(
            |(move_number, move_uci, fen_after, player, timestamp, blurred, think_ms, _)| MoveRow {
                move_number: *move_number,
                move_uci: move_uci.clone(),
                fen_after: fen_after.clone(),
                player: player.clone(),
                timestamp: *timestamp,
                blurred: *blurred != 0,
                think_ms: think_ms.map(|t| t.max(0) as u32),
            },
        )
        .collect();

    // Audit client think times against the server-observed wall clock before
    // they reach analysis. Client timing is accurate but forgeable; a side
    // whose claimed thinking can't fit the game's real duration has its think
    // times stripped (reverting it to server/none timing) rather than trusted.
    let reported_at: Vec<Option<i64>> = raw_moves.iter().map(|r| r.7).collect();
    audit_think_times(&mut rows, &reported_at, &game_id_str);

    let white_elo = fetch_elo(state, &game.white).await;
    let black_elo = fetch_elo(state, &game.black).await;

    let (tournament_id, tournament_round) =
        resolve_tournament_context(state, game.tournament_id, game.game_id).await;

    let meta = GameMeta {
        game_id: game_id_str.clone(),
        player_white: game.white.clone(),
        player_black: game.black.clone(),
        white_elo,
        black_elo,
        stake_amount: game.wager_lamports as f64 / 1e9,
        tournament_id,
        tournament_round,
        winner: game.winner.clone(),
        end_time: Some(chrono::Utc::now().timestamp()),
        time_base_sec: if game.base_time_seconds == 0 {
            600
        } else {
            game.base_time_seconds
        },
        time_inc_sec: game.increment_seconds,
    };

    let record = match build_game_record(&rows, &meta) {
        Ok(r) => r,
        Err(e) => {
            warn!("[anticheat] cannot build record for game {game_id_str}: {e}");
            return;
        }
    };

    // T0 tier: free casual games skip Stockfish unless the timing screen
    // finds something. Stakes-bearing games (wager or tournament) always get
    // full engine analysis.
    let stakes = game.wager_lamports > 0 || tournament_id.is_some();
    if !stakes {
        let screen = xfchess_anticheat::features::screen::t0_screen(&record);
        if !screen.suspicious {
            worker_metrics::ANTICHEAT_SCREENED_OUT_TOTAL.fetch_add(1, Ordering::Relaxed);
            // Persist the screen result so cross-game queries can see the
            // game was looked at. Never produces Review/Flag on its own.
            sqlx::query(
                "INSERT OR IGNORE INTO anticheat_verdicts
                 (game_id, engine_version, analysis_depth,
                  white_pubkey, black_pubkey, white_verdict, black_verdict,
                  white_score, black_score, white_signals, black_signals, report_path)
                 VALUES (?, 'screen-v1', 0, ?, ?, 'Clean', 'Clean', ?, ?, '{}', '{}', '')",
            )
            .bind(&game_id_str)
            .bind(&game.white)
            .bind(&game.black)
            .bind(screen.white_score)
            .bind(screen.black_score)
            .execute(&pool)
            .await
            .ok();
            info!("[anticheat] game {game_id_str} passed T0 screen — skipping engine analysis");
            return;
        }
        info!(
            "[anticheat] game {game_id_str} failed T0 screen (w {:.2} / b {:.2}) — full analysis",
            screen.white_score, screen.black_score
        );
    }

    // Durable queue row first: if the in-memory queue is full or the server
    // dies before analysis, the row marks the game as pending (the prize gate
    // reads it) and a future re-ingest sweep can pick it up.
    sqlx::query("INSERT OR IGNORE INTO anticheat_queue (game_id) VALUES (?)")
        .bind(&game_id_str)
        .execute(&pool)
        .await
        .ok();

    match queue.enqueue(record).await {
        Ok(_) => {
            worker_metrics::ANTICHEAT_ENQUEUED_TOTAL.fetch_add(1, Ordering::Relaxed);
            info!("[anticheat] game {game_id_str} queued for analysis");
        }
        Err(e) => {
            worker_metrics::ANTICHEAT_DROPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
            error!("[anticheat] failed to enqueue game {game_id_str}: {e}");
        }
    }
    worker_metrics::ANTICHEAT_QUEUE_DEPTH.store(queue.depth() as u64, Ordering::Relaxed);
}

/// Slack on the wall-clock budget (think times include UI/render lag and the
/// clock estimate is a lower bound).
const BUDGET_SLACK: f64 = 1.25;

/// Audits client think times against the server-observed game duration and
/// strips any side whose claimed thinking can't physically fit.
///
/// The wall clock is the larger of two server-observed spans: the move
/// timestamps (accurate for real-time submission, ~0 when batched) and the
/// telemetry `reported_at` arrivals (accurate even when moves are batched,
/// since each client reports its moves in real time). Using the larger keeps
/// honest batched games trustworthy while still catching a client that claims
/// more thinking than the game physically lasted.
fn audit_think_times(rows: &mut [MoveRow], reported_at: &[Option<i64>], game_id: &str) {
    // Strongest available wall-clock estimate, in ms.
    let move_span_ms = match (rows.first(), rows.last()) {
        (Some(f), Some(l)) => ((l.timestamp - f.timestamp).max(0) * 1000) as u64,
        _ => 0,
    };
    let reported: Vec<i64> = reported_at.iter().filter_map(|r| *r).collect();
    let telemetry_span_ms = match (reported.iter().min(), reported.iter().max()) {
        (Some(min), Some(max)) => ((max - min).max(0) * 1000) as u64,
        _ => 0,
    };
    let wall_ms = move_span_ms.max(telemetry_span_ms);
    if wall_ms == 0 {
        return; // No usable clock — leave think times for the resolver to judge.
    }
    let budget = (wall_ms as f64 * BUDGET_SLACK) as u64;

    // Per-side claimed-think sum. A side can't have thought longer than the
    // whole game; if it claims to have, its telemetry is fabricated.
    for parity in 0..2usize {
        let sum: u64 = rows
            .iter()
            .filter(|m| (m.move_number as usize % 2) == (1 - parity)) // odd=white(parity0)
            .filter_map(|m| m.think_ms.map(|t| t as u64))
            .sum();
        if sum > budget {
            let side = if parity == 0 { "white" } else { "black" };
            warn!(
                "[anticheat] game {game_id}: {side} think-time sum {}ms exceeds budget {}ms — discarding (tamper)",
                sum, budget
            );
            worker_metrics::TELEMETRY_DISCARDED_TOTAL.fetch_add(1, Ordering::Relaxed);
            for m in rows.iter_mut() {
                if (m.move_number as usize % 2) == (1 - parity) {
                    m.think_ms = None;
                }
            }
        }
    }
}

/// How often the re-ingest sweep retries dropped or orphaned queue rows.
const REINGEST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// Only retry rows at least this old — younger ones may still be in flight in
/// the in-memory queue, and re-enqueueing them would duplicate analysis.
const REINGEST_MIN_AGE_SECS: i64 = 10 * 60;
/// Rows retried per sweep.
const REINGEST_BATCH: i64 = 50;

/// Spawns the sweep that picks `anticheat_queue` rows back up after a queue
/// overflow or a server restart, rebuilding the job from the `games` table.
pub fn spawn_reingest_sweep(state: std::sync::Arc<AppState>) {
    if state.anticheat_queue.is_none() {
        return;
    }
    let max_attempts = xfchess_anticheat::config::AcConfig::from_env().max_attempts as i64;
    tokio::spawn(async move {
        info!(
            "[anticheat] re-ingest sweep started ({}s interval)",
            REINGEST_INTERVAL.as_secs()
        );
        let mut ticker = tokio::time::interval(REINGEST_INTERVAL);
        ticker.tick().await; // skip immediate first tick
        loop {
            ticker.tick().await;
            sweep_once(&state, max_attempts).await;
        }
    });
}

async fn sweep_once(state: &AppState, max_attempts: i64) {
    let pool = state.store.pool();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT q.game_id FROM anticheat_queue q
         LEFT JOIN anticheat_verdicts v ON v.game_id = q.game_id
         WHERE v.game_id IS NULL
           AND q.attempts < ?
           AND q.queued_at < strftime('%s','now') - ?
         ORDER BY q.queued_at ASC LIMIT ?",
    )
    .bind(max_attempts)
    .bind(REINGEST_MIN_AGE_SECS)
    .bind(REINGEST_BATCH)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (game_id_str,) in rows {
        let Ok(game_id) = game_id_str.parse::<u64>() else {
            continue;
        };
        // Rebuild the job from the completed game record.
        type GameRow = (Option<String>, Option<String>, Option<String>, f64);
        let game_row: Option<GameRow> = sqlx::query_as(
            "SELECT player_white, player_black, winner, stake_amount FROM games WHERE id = ?",
        )
        .bind(&game_id_str)
        .fetch_optional(&pool)
        .await
        .unwrap_or_default();

        let Some((Some(white), Some(black), winner, stake_sol)) = game_row else {
            // Nothing to rebuild from — count the attempt so the row ages out.
            sqlx::query(
                "UPDATE anticheat_queue SET attempts = attempts + 1,
                 last_error = 'no games row to rebuild from' WHERE game_id = ?",
            )
            .bind(&game_id_str)
            .execute(&pool)
            .await
            .ok();
            continue;
        };

        info!("[anticheat] re-ingesting game {game_id_str}");
        enqueue_game_analysis(
            state,
            FinalizedGame {
                game_id,
                white,
                black,
                winner,
                wager_lamports: (stake_sol * 1e9) as u64,
                tournament_id: None,
                base_time_seconds: 0,
                increment_seconds: 0,
            },
        )
        .await;
    }
}

async fn fetch_elo(state: &AppState, pubkey: &str) -> u32 {
    state
        .elo_cache
        .get_elo(pubkey)
        .await
        .map(|e| (e.elo_rating / 100.0) as u32)
        .unwrap_or(1200)
}

/// Resolves `(tournament_id, 1-based round)` for a game.
///
/// With an on-chain hint the round is looked up on that tournament's match
/// list; without one (the HTTP path doesn't carry it) every live tournament
/// is scanned for a match holding this game_id.
async fn resolve_tournament_context(
    state: &AppState,
    hint: Option<u64>,
    game_id: u64,
) -> (Option<u64>, Option<u32>) {
    fn round_in(
        t: &crate::signing::storage::tournament::TournamentRecord,
        game_id: u64,
    ) -> Option<u32> {
        t.matches
            .iter()
            .flatten()
            .find(|m| m.game_id == Some(game_id))
            .map(|m| m.round as u32 + 1)
    }

    if let Some(tid) = hint {
        let round = match state.tournament_store.get(tid).await {
            Some(t) => round_in(&t, game_id),
            None => None,
        };
        return (Some(tid), round);
    }

    for t in state.tournament_store.list().await {
        if let Some(round) = round_in(&t, game_id) {
            return (Some(t.tournament_id), Some(round));
        }
    }
    (None, None)
}
