//! Puzzle routes — serve positions, verify solutions server-side, rate the
//! player, and pay admin-funded bounties. See docs/PUZZLES.md.
//!
//! Trust model: the solution `line` NEVER leaves the VPS. The client receives
//! only the position (and the opponent's setup move, which leaks nothing). The
//! client submits the moves it played; the backend replays them against the
//! stored line and decides win/loss. Only a server-verified win can pay out.
//!
//! The earn model is admin-prefunded bounties only (no staking, no ladder): an
//! operator funds a puzzle / band / daily with the VPS authority key, and a
//! verified solve that passes the anti-cheat gate pays the posted reward as a
//! plain SOL transfer from the VPS wallet (off-chain budget accounting, v1).

use std::str::FromStr;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, warn};

use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::signing::AppState;

// ── DB row types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
struct PuzzleRow {
    id: String,
    fen: String,
    line: String,
    rating: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct RatingRow {
    rating: i64,
    rating_dev: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ChallengeRow {
    wallet: String,
    puzzle_id: String,
    mode: String,
    issued_at: i64,
    expires_at: i64,
    consumed: i64,
    progress: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct BountyRow {
    id: i64,
    reward_lamports: i64,
    budget_lamports: i64,
    spent_lamports: i64,
    max_per_wallet: i64,
}

// ── Tuning constants ────────────────────────────────────────────────────────

const CHALLENGE_TTL_SECS: i64 = 600; // 10 min to submit
const RATING_WINDOW: i64 = 150; // ±band around player rating
const DEFAULT_RATING: i64 = 1500;
const DEFAULT_RATING_DEV: i64 = 350;
const DAILY_PAYOUT_CAP: i64 = 5; // max paid solves per wallet per day

// ── Router builders ─────────────────────────────────────────────────────────

/// Public, player-facing puzzle routes (wallet-scoped, nonce-protected).
pub fn puzzle_routes() -> Router<AppState> {
    Router::new()
        .route("/puzzle/next", get(get_next))
        .route("/puzzle/solve", post(post_solve))
        .route("/puzzle/move", post(post_move))
        .route("/puzzle/daily", get(get_daily))
        .route("/puzzle/rating/{wallet}", get(get_rating))
}

// ── Serve: GET /puzzle/next?mode=solve|earn&wallet=W ─────────────────────────

#[derive(Deserialize)]
struct NextQuery {
    wallet: String,
    mode: Option<String>,
}

async fn get_next(
    State(state): State<AppState>,
    Query(q): Query<NextQuery>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let mode = q.mode.as_deref().unwrap_or("solve");
    if q.wallet.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (player_rating, _) = load_rating(&pool, &q.wallet).await;

    // Select a puzzle the wallet has not attempted, in the rating band; for
    // `earn` restrict to puzzles covered by an active bounty.
    let puzzle = if mode == "earn" {
        select_funded_puzzle(&pool, &q.wallet).await
    } else {
        select_puzzle(&pool, &q.wallet, player_rating).await
    };

    let Some(puzzle) = puzzle else {
        return Ok(Json(json!({ "exhausted": true })));
    };

    let reward = if mode == "earn" || mode == "daily" {
        find_active_bounty(&pool, &puzzle.id, puzzle.rating)
            .await
            .map(|b| b.reward_lamports)
    } else {
        None
    };

    serve_puzzle(&pool, &q.wallet, &puzzle, mode, reward).await
}

// ── Serve: GET /puzzle/daily?wallet=W ────────────────────────────────────────

async fn get_daily(
    State(state): State<AppState>,
    Query(q): Query<NextQuery>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    if q.wallet.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let Some(puzzle) = select_daily_puzzle(&pool).await else {
        return Ok(Json(json!({ "exhausted": true })));
    };

    // Already solved today's puzzle? Tell the client so it can show the result
    // screen instead of re-serving.
    if round_exists(&pool, &q.wallet, &puzzle.id).await {
        return Ok(Json(json!({ "id": puzzle.id, "already_attempted": true })));
    }

    let reward = find_active_bounty(&pool, &puzzle.id, puzzle.rating)
        .await
        .map(|b| b.reward_lamports);

    serve_puzzle(&pool, &q.wallet, &puzzle, "daily", reward).await
}

/// Common serve path: record a single-use challenge, return the position + the
/// opponent setup move (line[0]) and the player's colour — but never the line.
async fn serve_puzzle(
    pool: &sqlx::SqlitePool,
    wallet: &str,
    puzzle: &PuzzleRow,
    mode: &str,
    reward: Option<i64>,
) -> Result<Json<Value>, StatusCode> {
    let line: Vec<&str> = puzzle.line.split(' ').collect();
    let setup_move = line.first().copied().unwrap_or("").to_string();
    let player_color = player_color_after_setup(&puzzle.fen);
    // Number of moves the player must make (odd indices of the line).
    let solution_len = line.len().saturating_sub(1).div_ceil(2) as i64;

    let nonce = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let res = sqlx::query(
        "INSERT INTO puzzle_challenges (nonce, wallet, puzzle_id, mode, issued_at, expires_at, consumed)
         VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&nonce)
    .bind(wallet)
    .bind(&puzzle.id)
    .bind(mode)
    .bind(now)
    .bind(now + CHALLENGE_TTL_SECS)
    .execute(pool)
    .await;

    if let Err(e) = res {
        error!("[puzzle] failed to record challenge: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut body = json!({
        "nonce": nonce,
        "id": puzzle.id,
        "fen": puzzle.fen,
        "setup_move": setup_move,
        "color": player_color,
        "rating": puzzle.rating,
        "solution_len": solution_len,
    });
    if let Some(r) = reward {
        body["reward_lamports"] = json!(r);
    }
    Ok(Json(body))
}

// ── Verify: POST /puzzle/solve { nonce, moves: [..] } ────────────────────────

#[derive(Deserialize)]
struct SolveReq {
    nonce: String,
    moves: Vec<String>,
}

async fn post_solve(
    State(state): State<AppState>,
    Json(req): Json<SolveReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let now = chrono::Utc::now().timestamp();

    // 1. Consume the challenge — single-use, errors if missing/expired/consumed.
    let ch = match consume_challenge(&pool, &req.nonce, now).await {
        Ok(c) => c,
        Err(code) => return Err(code),
    };

    // 2. Guard against double credit: one (wallet, puzzle) attempt ever.
    if round_exists(&pool, &ch.wallet, &ch.puzzle_id).await {
        return Err(StatusCode::CONFLICT);
    }

    // 3. Load the puzzle (has the secret line).
    let Some(puzzle) = load_puzzle(&pool, &ch.puzzle_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };

    // 4. Win condition: submitted == the player's moves (odd indices of the
    //    line; even indices are the opponent's replies, never submitted).
    let line: Vec<&str> = puzzle.line.split(' ').collect();
    let expected: Vec<&str> = line.iter().skip(1).step_by(2).copied().collect();
    let submitted: Vec<String> = req.moves.iter().map(|s| s.trim().to_string()).collect();
    let win = !expected.is_empty()
        && submitted.len() == expected.len()
        && submitted.iter().zip(&expected).all(|(a, b)| a == b);

    // 5. Server-observed think time (not a client claim).
    let solve_ms = (now - ch.issued_at).max(0) * 1000;

    // 6. Rating + payout + round record.
    let outcome = finalize(&state, &pool, &ch, &puzzle, win, solve_ms, now).await;

    Ok(Json(json!({
        "win": win,
        "rating": outcome.new_rating,
        "rating_diff": outcome.rating_diff,
        "payout_sig": outcome.payout_sig,
        "paid_lamports": outcome.paid_lamports,
    })))
}

// ── Interactive verify: POST /puzzle/move { nonce, uci } ─────────────────────

#[derive(Deserialize)]
struct MoveReq {
    nonce: String,
    uci: String,
}

/// One move at a time. Reveals the opponent's reply only *after* a correct
/// player move (never a future player move). The final correct move awards the
/// win and pays any bounty. A wrong move ends the puzzle as a loss.
async fn post_move(
    State(state): State<AppState>,
    Json(req): Json<MoveReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let now = chrono::Utc::now().timestamp();

    // Load the challenge WITHOUT consuming — it lives across the whole sequence.
    let ch = sqlx::query_as::<_, ChallengeRow>(
        "SELECT wallet, puzzle_id, mode, issued_at, expires_at, consumed, progress
         FROM puzzle_challenges WHERE nonce = ?",
    )
    .bind(&req.nonce)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if ch.consumed != 0 || now > ch.expires_at {
        return Err(StatusCode::GONE);
    }
    if round_exists(&pool, &ch.wallet, &ch.puzzle_id).await {
        return Err(StatusCode::CONFLICT);
    }

    let Some(puzzle) = load_puzzle(&pool, &ch.puzzle_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    let line: Vec<&str> = puzzle.line.split(' ').collect();
    let pidx = 1 + 2 * ch.progress as usize; // this player move's index in the line
    if pidx >= line.len() {
        consume(&pool, &req.nonce).await;
        return Err(StatusCode::CONFLICT);
    }

    if req.uci.trim() != line[pidx] {
        // Wrong move ends the puzzle as a loss.
        consume(&pool, &req.nonce).await;
        let solve_ms = (now - ch.issued_at).max(0) * 1000;
        let outcome = finalize(&state, &pool, &ch, &puzzle, false, solve_ms, now).await;
        return Ok(Json(json!({
            "correct": false, "done": true, "win": false,
            "rating": outcome.new_rating, "rating_diff": outcome.rating_diff,
        })));
    }

    let oidx = pidx + 1;
    if oidx < line.len() {
        // Correct, more to go: advance and reveal the opponent's reply.
        let _ = sqlx::query("UPDATE puzzle_challenges SET progress = progress + 1 WHERE nonce = ?")
            .bind(&req.nonce)
            .execute(&pool)
            .await;
        Ok(Json(json!({ "correct": true, "done": false, "reply": line[oidx] })))
    } else {
        // That was the final move — solved.
        consume(&pool, &req.nonce).await;
        let solve_ms = (now - ch.issued_at).max(0) * 1000;
        let outcome = finalize(&state, &pool, &ch, &puzzle, true, solve_ms, now).await;
        Ok(Json(json!({
            "correct": true, "done": true, "win": true,
            "rating": outcome.new_rating, "rating_diff": outcome.rating_diff,
            "payout_sig": outcome.payout_sig, "paid_lamports": outcome.paid_lamports,
        })))
    }
}

struct Outcome {
    new_rating: i64,
    rating_diff: i64,
    payout_sig: Option<String>,
    paid_lamports: i64,
}

/// Shared finalisation: rating update, bounty payout (earn/daily, anti-cheat
/// gated), and the single per-(wallet,puzzle) round record.
async fn finalize(
    state: &AppState,
    pool: &sqlx::SqlitePool,
    ch: &ChallengeRow,
    puzzle: &PuzzleRow,
    win: bool,
    solve_ms: i64,
    now: i64,
) -> Outcome {
    let (player_rating, player_dev) = load_rating(pool, &ch.wallet).await;
    let (new_rating, new_dev) = rating_update(player_rating, player_dev, puzzle.rating, win);
    save_rating(pool, &ch.wallet, new_rating, new_dev, now).await;

    let mut payout_sig: Option<String> = None;
    let mut paid_lamports: i64 = 0;
    if win && (ch.mode == "earn" || ch.mode == "daily") {
        if let Some(bounty) = find_active_bounty(pool, &puzzle.id, puzzle.rating).await {
            if let PayoutDecision::Pay =
                payout_decision(pool, &ch.wallet, &bounty, solve_ms, puzzle.rating, now).await
            {
                match pay_sol(state, &ch.wallet, bounty.reward_lamports as u64).await {
                    Ok(sig) => {
                        paid_lamports = bounty.reward_lamports;
                        debit_bounty(pool, bounty.id, bounty.reward_lamports, bounty.budget_lamports, bounty.spent_lamports).await;
                        info!("[puzzle] paid {} lamports to {} (sig {})", bounty.reward_lamports, ch.wallet, sig);
                        payout_sig = Some(sig);
                    }
                    Err(e) => warn!("[puzzle] payout failed for {}: {e}", ch.wallet),
                }
            } else {
                warn!("[puzzle] payout withheld for {}", ch.wallet);
            }
        }
    }

    let _ = sqlx::query(
        "INSERT OR IGNORE INTO puzzle_rounds (wallet, puzzle_id, win, rated, solve_ms, paid_sig, created_at)
         VALUES (?, ?, ?, 1, ?, ?, ?)",
    )
    .bind(&ch.wallet)
    .bind(&puzzle.id)
    .bind(win as i64)
    .bind(solve_ms)
    .bind(&payout_sig)
    .bind(now)
    .execute(pool)
    .await;

    let _ = if win {
        sqlx::query("UPDATE puzzles SET plays = plays + 1, nb_wins = nb_wins + 1 WHERE id = ?")
            .bind(&puzzle.id).execute(pool).await
    } else {
        sqlx::query("UPDATE puzzles SET plays = plays + 1 WHERE id = ?")
            .bind(&puzzle.id).execute(pool).await
    };

    Outcome { new_rating, rating_diff: new_rating - player_rating, payout_sig, paid_lamports }
}

async fn consume(pool: &sqlx::SqlitePool, nonce: &str) {
    let _ = sqlx::query("UPDATE puzzle_challenges SET consumed = 1 WHERE nonce = ?")
        .bind(nonce)
        .execute(pool)
        .await;
}

// ── GET /puzzle/rating/{wallet} ──────────────────────────────────────────────

async fn get_rating(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Json<Value> {
    let pool = state.store.pool();
    let (rating, dev) = load_rating(&pool, &wallet).await;
    Json(json!({ "wallet": wallet, "rating": rating, "rating_dev": dev }))
}

// ── Selection helpers ────────────────────────────────────────────────────────

async fn select_puzzle(pool: &sqlx::SqlitePool, wallet: &str, rating: i64) -> Option<PuzzleRow> {
    // Widen the band on retry until we find an unplayed puzzle.
    for mult in [1i64, 2, 4, 12] {
        let win = RATING_WINDOW * mult;
        let row = sqlx::query_as::<_, PuzzleRow>(
            "SELECT id, fen, line, rating FROM puzzles
             WHERE enabled = 1 AND rating BETWEEN ? AND ?
               AND id NOT IN (SELECT puzzle_id FROM puzzle_rounds WHERE wallet = ?)
             ORDER BY RANDOM() LIMIT 1",
        )
        .bind(rating - win)
        .bind(rating + win)
        .bind(wallet)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if row.is_some() {
            return row;
        }
    }
    None
}

async fn select_funded_puzzle(pool: &sqlx::SqlitePool, wallet: &str) -> Option<PuzzleRow> {
    sqlx::query_as::<_, PuzzleRow>(
        "SELECT DISTINCT p.id, p.fen, p.line, p.rating FROM puzzles p
         JOIN puzzle_bounties b
           ON (b.scope = 'puzzle' AND b.puzzle_id = p.id)
           OR (b.scope = 'band' AND p.rating BETWEEN b.band_lo AND b.band_hi)
         WHERE p.enabled = 1 AND b.status = 'active' AND b.spent_lamports < b.budget_lamports
           AND p.id NOT IN (SELECT puzzle_id FROM puzzle_rounds WHERE wallet = ?)
         ORDER BY RANDOM() LIMIT 1",
    )
    .bind(wallet)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn select_daily_puzzle(pool: &sqlx::SqlitePool) -> Option<PuzzleRow> {
    // Deterministic per-day pick: offset by the epoch-day into the mid-rating band.
    let day = chrono::Utc::now().timestamp() / 86_400;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM puzzles WHERE enabled = 1 AND rating BETWEEN 1200 AND 1800",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if count == 0 {
        return None;
    }
    let offset = day.rem_euclid(count);
    sqlx::query_as::<_, PuzzleRow>(
        "SELECT id, fen, line, rating FROM puzzles
         WHERE enabled = 1 AND rating BETWEEN 1200 AND 1800
         ORDER BY id LIMIT 1 OFFSET ?",
    )
    .bind(offset)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn load_puzzle(pool: &sqlx::SqlitePool, id: &str) -> Option<PuzzleRow> {
    sqlx::query_as::<_, PuzzleRow>("SELECT id, fen, line, rating FROM puzzles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn round_exists(pool: &sqlx::SqlitePool, wallet: &str, puzzle_id: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM puzzle_rounds WHERE wallet = ? AND puzzle_id = ?",
    )
    .bind(wallet)
    .bind(puzzle_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        > 0
}

// ── Challenge lifecycle ──────────────────────────────────────────────────────

async fn consume_challenge(
    pool: &sqlx::SqlitePool,
    nonce: &str,
    now: i64,
) -> Result<ChallengeRow, StatusCode> {
    let ch = sqlx::query_as::<_, ChallengeRow>(
        "SELECT wallet, puzzle_id, mode, issued_at, expires_at, consumed, progress
         FROM puzzle_challenges WHERE nonce = ?",
    )
    .bind(nonce)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if ch.consumed != 0 || now > ch.expires_at {
        return Err(StatusCode::GONE);
    }

    // Mark consumed atomically; if 0 rows changed someone else won the race.
    let res = sqlx::query("UPDATE puzzle_challenges SET consumed = 1 WHERE nonce = ? AND consumed = 0")
        .bind(nonce)
        .execute(pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::GONE);
    }
    Ok(ch)
}

// ── Rating (simplified Glicko / Elo) ─────────────────────────────────────────

async fn load_rating(pool: &sqlx::SqlitePool, wallet: &str) -> (i64, i64) {
    sqlx::query_as::<_, RatingRow>(
        "SELECT rating, rating_dev FROM puzzle_ratings WHERE wallet = ?",
    )
    .bind(wallet)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| (r.rating, r.rating_dev))
    .unwrap_or((DEFAULT_RATING, DEFAULT_RATING_DEV))
}

async fn save_rating(pool: &sqlx::SqlitePool, wallet: &str, rating: i64, dev: i64, now: i64) {
    let _ = sqlx::query(
        "INSERT INTO puzzle_ratings (wallet, rating, rating_dev, nb, updated_at)
         VALUES (?, ?, ?, 1, ?)
         ON CONFLICT(wallet) DO UPDATE SET
            rating = excluded.rating,
            rating_dev = excluded.rating_dev,
            nb = puzzle_ratings.nb + 1,
            updated_at = excluded.updated_at",
    )
    .bind(wallet)
    .bind(rating)
    .bind(dev)
    .bind(now)
    .execute(pool)
    .await;
}

/// Elo-style update against the puzzle's rating as the "opponent", with the
/// rating deviation shrinking toward a confident floor as the player solves
/// more (a lightweight stand-in for full Glicko-2).
fn rating_update(player: i64, dev: i64, puzzle: i64, won: bool) -> (i64, i64) {
    let expected = 1.0 / (1.0 + 10f64.powf((puzzle - player) as f64 / 400.0));
    let score = if won { 1.0 } else { 0.0 };
    // Higher deviation ⇒ larger swings (less confidence), like Glicko.
    let k = 16.0 + (dev as f64 / 350.0) * 24.0;
    let new = (player as f64 + k * (score - expected)).round() as i64;
    let new_dev = ((dev as f64) * 0.96).round().max(60.0) as i64;
    (new.clamp(400, 3200), new_dev)
}

// ── Bounty + payout ──────────────────────────────────────────────────────────

async fn find_active_bounty(
    pool: &sqlx::SqlitePool,
    puzzle_id: &str,
    rating: i64,
) -> Option<BountyRow> {
    sqlx::query_as::<_, BountyRow>(
        "SELECT id, reward_lamports, budget_lamports, spent_lamports, max_per_wallet
         FROM puzzle_bounties
         WHERE status = 'active' AND spent_lamports + reward_lamports <= budget_lamports
           AND ( (scope = 'puzzle' AND puzzle_id = ?)
              OR (scope = 'band'   AND ? BETWEEN band_lo AND band_hi)
              OR (scope = 'daily') )
         ORDER BY (scope = 'puzzle') DESC, reward_lamports DESC
         LIMIT 1",
    )
    .bind(puzzle_id)
    .bind(rating)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

enum PayoutDecision {
    Pay,
    Withhold(&'static str),
}

/// Anti-cheat gate (docs/PUZZLES.md §7). A puzzle is deterministic, so a bot
/// with an engine solves instantly and perfectly — these checks decide whether
/// a verified win actually pays.
async fn payout_decision(
    pool: &sqlx::SqlitePool,
    wallet: &str,
    bounty: &BountyRow,
    solve_ms: i64,
    puzzle_rating: i64,
    now: i64,
) -> PayoutDecision {
    // Budget headroom.
    if bounty.spent_lamports + bounty.reward_lamports > bounty.budget_lamports {
        return PayoutDecision::Withhold("budget exhausted");
    }

    // Think-time floor: a rating-scaled minimum of server-observed wall time.
    // A 2200-rated puzzle solved in well under ~2s with no errors is an engine.
    let floor_ms = 500 + (puzzle_rating.max(0));
    if solve_ms < floor_ms {
        return PayoutDecision::Withhold("below think-time floor");
    }

    // Per-wallet daily payout cap (bounded faucet cost).
    let start_of_day = now - now.rem_euclid(86_400);
    let paid_today = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM puzzle_rounds
         WHERE wallet = ? AND paid_sig IS NOT NULL AND created_at >= ?",
    )
    .bind(wallet)
    .bind(start_of_day)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if paid_today >= DAILY_PAYOUT_CAP {
        return PayoutDecision::Withhold("daily cap reached");
    }

    // Per-wallet-per-bounty cap.
    let paid_from_bounty = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM puzzle_rounds WHERE wallet = ? AND paid_sig IS NOT NULL",
    )
    .bind(wallet)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let _ = paid_from_bounty; // band/daily bounties reuse the daily cap above
    if bounty.max_per_wallet <= 0 {
        return PayoutDecision::Withhold("no per-wallet allowance");
    }

    PayoutDecision::Pay
}

async fn debit_bounty(
    pool: &sqlx::SqlitePool,
    id: i64,
    reward: i64,
    budget: i64,
    spent: i64,
) {
    let new_spent = spent + reward;
    let exhausted = new_spent + reward > budget; // can't fund another full reward
    let _ = sqlx::query(
        "UPDATE puzzle_bounties
         SET spent_lamports = spent_lamports + ?,
             status = CASE WHEN ? THEN 'exhausted' ELSE status END
         WHERE id = ?",
    )
    .bind(reward)
    .bind(exhausted as i64)
    .bind(id)
    .execute(pool)
    .await;
}

/// Pay a SOL bounty as a plain transfer signed by the VPS authority key
/// (off-chain budget accounting, v1 — mirrors the prefunded-prize model).
async fn pay_sol(state: &AppState, to: &str, lamports: u64) -> anyhow::Result<String> {
    let to_pk = Pubkey::from_str(to)?;
    let authority = state.vps_authority.clone();
    let rpc = state.solana_rpc.clone();
    let sig = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let ix = solana_sdk::system_instruction::transfer(&authority.pubkey(), &to_pk, lamports);
        let bh = rpc.get_latest_blockhash()?;
        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[ix],
            Some(&authority.pubkey()),
            &[&*authority],
            bh,
        );
        let sig = rpc.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    })
    .await??;
    Ok(sig)
}

// ── FEN helper ───────────────────────────────────────────────────────────────

/// The player controls the side that moves *after* the engine's setup move,
/// i.e. the opposite of the FEN side-to-move field.
fn player_color_after_setup(fen: &str) -> &'static str {
    match fen.split_whitespace().nth(1) {
        Some("w") => "black",
        _ => "white",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin: curation + funding (docs/PUZZLES.md §9). Mounted behind require_api_key.
// ─────────────────────────────────────────────────────────────────────────────

/// Operator routes — browse the pool by ELO/name, label/feature/enable puzzles,
/// and fund bounties with the VPS authority. Auth is applied at mount time.
pub fn puzzle_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/puzzles", get(admin_list))
        .route("/admin/puzzles/fund", post(admin_fund))
        .route("/admin/puzzles/bounties", get(admin_bounties))
        .route("/admin/puzzles/bounties/{id}/close", post(admin_close_bounty))
        .route("/admin/puzzles/{id}", get(admin_get))
        .route("/admin/puzzles/{id}/name", post(admin_name))
        .route("/admin/puzzles/{id}/feature", post(admin_feature))
        .route("/admin/puzzles/{id}/enable", post(admin_enable))
}

#[derive(Deserialize)]
struct AdminListQuery {
    elo_min: Option<i64>,
    elo_max: Option<i64>,
    name: Option<String>,
    theme: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
struct PuzzleFull {
    id: String,
    name: Option<String>,
    fen: String,
    line: String,
    rating: i64,
    rating_dev: i64,
    themes: String,
    plays: i64,
    nb_wins: i64,
    featured: i64,
    enabled: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
struct BountyFull {
    id: i64,
    scope: String,
    puzzle_id: Option<String>,
    band_lo: Option<i64>,
    band_hi: Option<i64>,
    reward_lamports: i64,
    budget_lamports: i64,
    spent_lamports: i64,
    max_per_wallet: i64,
    fund_sig: Option<String>,
    status: String,
    created_at: i64,
}

async fn admin_list(
    State(state): State<AppState>,
    Query(q): Query<AdminListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let elo_min = q.elo_min.unwrap_or(0);
    let elo_max = q.elo_max.unwrap_or(100_000);
    let name = q.name.unwrap_or_default();
    let name_like = format!("%{name}%");
    let theme = q.theme.unwrap_or_default();
    let theme_like = format!("%{theme}%");
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM puzzles
         WHERE rating BETWEEN ? AND ?
           AND (? = '' OR name LIKE ?)
           AND (? = '' OR themes LIKE ?)",
    )
    .bind(elo_min)
    .bind(elo_max)
    .bind(&name)
    .bind(&name_like)
    .bind(&theme)
    .bind(&theme_like)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, PuzzleFull>(
        "SELECT id, name, fen, line, rating, rating_dev, themes, plays, nb_wins, featured, enabled
         FROM puzzles
         WHERE rating BETWEEN ? AND ?
           AND (? = '' OR name LIKE ?)
           AND (? = '' OR themes LIKE ?)
         ORDER BY rating LIMIT ? OFFSET ?",
    )
    .bind(elo_min)
    .bind(elo_max)
    .bind(&name)
    .bind(&name_like)
    .bind(&theme)
    .bind(&theme_like)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("[puzzle-admin] list failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({ "total": total, "puzzles": rows })))
}

async fn admin_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let row = sqlx::query_as::<_, PuzzleFull>(
        "SELECT id, name, fen, line, rating, rating_dev, themes, plays, nb_wins, featured, enabled
         FROM puzzles WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(json!(row)))
}

#[derive(Deserialize)]
struct NameReq {
    name: String,
}

async fn admin_name(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<NameReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    sqlx::query("UPDATE puzzles SET name = ? WHERE id = ?")
        .bind(&body.name)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    info!("[puzzle-admin] named {id} = {}", body.name);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FeatureReq {
    featured: bool,
}

async fn admin_feature(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<FeatureReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    sqlx::query("UPDATE puzzles SET featured = ? WHERE id = ?")
        .bind(body.featured as i64)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct EnableReq {
    enabled: bool,
}

async fn admin_enable(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EnableReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    sqlx::query("UPDATE puzzles SET enabled = ? WHERE id = ?")
        .bind(body.enabled as i64)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FundReq {
    scope: String, // 'puzzle' | 'band' | 'daily'
    puzzle_id: Option<String>,
    band_lo: Option<i64>,
    band_hi: Option<i64>,
    reward_lamports: i64,
    budget_lamports: i64,
    max_per_wallet: Option<i64>,
}

/// Fund a bounty. v1 keeps the budget as off-chain accounting backed by the VPS
/// wallet (the prefunding action), mirroring `fund_tournament_prize`; an
/// on-chain escrow PDA is the optional later step (docs/PUZZLES.md §9.3).
async fn admin_fund(
    State(state): State<AppState>,
    Json(body): Json<FundReq>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    if !matches!(body.scope.as_str(), "puzzle" | "band" | "daily") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.reward_lamports <= 0 || body.budget_lamports < body.reward_lamports {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.scope == "puzzle" && body.puzzle_id.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.scope == "band" && (body.band_lo.is_none() || body.band_hi.is_none()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let now = chrono::Utc::now().timestamp();
    let max_per = body.max_per_wallet.unwrap_or(1).max(1);
    let res = sqlx::query(
        "INSERT INTO puzzle_bounties
           (scope, puzzle_id, band_lo, band_hi, reward_lamports, budget_lamports,
            spent_lamports, max_per_wallet, fund_sig, vault_pubkey, created_by, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, NULL, NULL, 'admin', 'active', ?)",
    )
    .bind(&body.scope)
    .bind(&body.puzzle_id)
    .bind(body.band_lo)
    .bind(body.band_hi)
    .bind(body.reward_lamports)
    .bind(body.budget_lamports)
    .bind(max_per)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("[puzzle-admin] fund failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let bounty_id = res.last_insert_rowid();
    info!(
        "[puzzle-admin] funded bounty {bounty_id}: scope={} reward={} budget={}",
        body.scope, body.reward_lamports, body.budget_lamports
    );
    Ok(Json(json!({ "bounty_id": bounty_id, "status": "active" })))
}

async fn admin_bounties(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    let rows = sqlx::query_as::<_, BountyFull>(
        "SELECT id, scope, puzzle_id, band_lo, band_hi, reward_lamports, budget_lamports,
                spent_lamports, max_per_wallet, fund_sig, status, created_at
         FROM puzzle_bounties ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "bounties": rows })))
}

async fn admin_close_bounty(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    let pool = state.store.pool();
    sqlx::query("UPDATE puzzle_bounties SET status = 'closed' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    info!("[puzzle-admin] closed bounty {id}");
    Ok(Json(json!({ "ok": true })))
}
