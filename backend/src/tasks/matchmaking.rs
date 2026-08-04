//! Matchmaking service for the XFChess backend.
//!
//! ELO window expands over time: ±150 base + 50 per 30 s of waiting.
//! Players who have waited the longest get priority when two tickets could
//! pair with the same opponent.

use crate::signing::routes::matchmaking::{MatchResult, SharedMatchmakingState};
use std::collections::HashSet;
use tracing::error;
use tracing::info;

/// How often the matching loop fires (seconds).
pub const MATCHMAKING_INTERVAL_SECONDS: u64 = 5;

/// Minimum players required to form a match.
pub const MATCHMAKING_MIN_PLAYERS: usize = 2;

/// Base ELO window at t=0.
const ELO_BASE: u32 = 150;
/// ELO added per 30 s of waiting.
const ELO_PER_STEP: u32 = 50;
/// Cap: after 5 minutes the window stops expanding (hard max ≈ 650 centiscale).
const ELO_MAX: u32 = 650;

/// Drop a queued ticket if nobody paired with it and the client never called
/// `/leave` (e.g. tab closed, connection dropped) after this long.
const STALE_QUEUE_SECS: u64 = 300;
/// Drop a match result if the player never came back to `/status` and
/// collect it (e.g. crashed right after being paired) after this long.
const STALE_MATCH_SECS: u64 = 300;

/// Return the allowed ELO difference for a ticket that joined `wait_secs` ago.
fn elo_window(wait_secs: u64) -> u32 {
    let steps = wait_secs / 30;
    (ELO_BASE + steps as u32 * ELO_PER_STEP).min(ELO_MAX)
}

/// Runs the matchmaking service.
///
/// Each iteration pairs the longest-waiting players that fall within each
/// other's current ELO window. Both players' windows must include the other
/// (the narrower window wins to keep games fair).
pub async fn run_matchmaking_service(state: SharedMatchmakingState) {
    // Reload queue/pending matches from SQLite so a backend restart resumes
    // instead of losing them (migration 022) — must happen before the first
    // tick so the sweep below doesn't see an empty queue.
    state.hydrate().await;

    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(MATCHMAKING_INTERVAL_SECONDS));

    loop {
        interval.tick().await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Sweep stale match results every tick, independent of queue size —
        // otherwise a player who crashes right after being paired leaves
        // their (and their opponent's) MatchResult in memory forever.
        match state.matches.lock() {
            Ok(mut matches) => {
                matches.retain(|_, m| now.saturating_sub(m.matched_at) < STALE_MATCH_SECS);
            }
            Err(e) => error!("[MATCHMAKING] Matches mutex poisoned during sweep: {}", e),
        }
        // Guard is dropped above, so this can safely await (std Mutex guards
        // aren't Send and can't cross an .await point in a spawned task).
        if let Err(e) = sqlx::query("DELETE FROM matchmaking_matches WHERE (? - matched_at) >= ?")
            .bind(now as i64)
            .bind(STALE_MATCH_SECS as i64)
            .execute(&state.pool)
            .await
        {
            error!(
                "[MATCHMAKING] Failed to sweep stale persisted matches: {}",
                e
            );
        }

        // Mutate the in-memory queue synchronously inside this block (no
        // .await while the mutex is held), then persist the outcome after
        // the guard drops.
        let (removed_stale, new_matches): (Vec<String>, Vec<(_, _, u64)>) = {
            let mut queue = match state.queue.lock() {
                Ok(q) => q,
                Err(e) => {
                    error!("[MATCHMAKING] Queue mutex poisoned: {}", e);
                    continue;
                }
            };

            // Drop tickets nobody ever paired with and the client never left
            // (dropped connection, closed tab) instead of waiting forever.
            let mut removed_stale = Vec::new();
            queue.retain(|t| {
                let keep = now.saturating_sub(t.joined_at) < STALE_QUEUE_SECS;
                if !keep {
                    removed_stale.push(t.pubkey.clone());
                }
                keep
            });

            if queue.len() < MATCHMAKING_MIN_PLAYERS {
                (removed_stale, Vec::new())
            } else {
                // Sort by join time (oldest first) so long-waiters get matched first.
                queue.sort_by_key(|t| t.joined_at);

                let mut matched_indices = HashSet::new();
                let mut new_matches = vec![];

                for i in 0..queue.len() {
                    if matched_indices.contains(&i) {
                        continue;
                    }
                    let wait_i = now.saturating_sub(queue[i].joined_at);
                    let window_i = elo_window(wait_i);

                    for j in (i + 1)..queue.len() {
                        if matched_indices.contains(&j) {
                            continue;
                        }
                        let wait_j = now.saturating_sub(queue[j].joined_at);
                        let window_j = elo_window(wait_j);

                        // Both players must accept each other's ELO (use the smaller window).
                        let allowed = window_i.min(window_j);
                        let diff = queue[i].elo.abs_diff(queue[j].elo);

                        if diff <= allowed {
                            let game_id = rand::random::<u64>();
                            info!(
                                "[Matchmaking] Pairing {} (ELO {}, wait {}s, window ±{}) vs {} (ELO {}, wait {}s, window ±{}) diff={}",
                                queue[i].pubkey, queue[i].elo, wait_i, window_i,
                                queue[j].pubkey, queue[j].elo, wait_j, window_j,
                                diff
                            );
                            new_matches.push((queue[i].clone(), queue[j].clone(), game_id));
                            matched_indices.insert(i);
                            matched_indices.insert(j);
                            break;
                        }
                    }
                }

                // Rebuild queue without matched players.
                let new_queue = queue
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| !matched_indices.contains(idx))
                    .map(|(_, t)| t.clone())
                    .collect();
                *queue = new_queue;

                (removed_stale, new_matches)
            }
        };

        for pubkey in &removed_stale {
            if let Err(e) = sqlx::query("DELETE FROM matchmaking_queue WHERE pubkey = ?")
                .bind(pubkey)
                .execute(&state.pool)
                .await
            {
                error!(
                    "[MATCHMAKING] Failed to sweep stale persisted queue ticket: {}",
                    e
                );
            }
        }

        // Save results to match map, then persist (queue rows for the
        // matched players cleared, match rows written).
        if !new_matches.is_empty() {
            match state.matches.lock() {
                Ok(mut matches) => {
                    for (p1, p2, game_id) in &new_matches {
                        matches.insert(
                            p1.pubkey.clone(),
                            MatchResult {
                                game_id: *game_id,
                                opponent: p2.pubkey.clone(),
                                is_white: true,
                                matched_at: now,
                            },
                        );
                        matches.insert(
                            p2.pubkey.clone(),
                            MatchResult {
                                game_id: *game_id,
                                opponent: p1.pubkey.clone(),
                                is_white: false,
                                matched_at: now,
                            },
                        );
                    }
                }
                Err(e) => error!("[MATCHMAKING] Matches mutex poisoned: {}", e),
            }

            for (p1, p2, game_id) in &new_matches {
                for pubkey in [&p1.pubkey, &p2.pubkey] {
                    if let Err(e) = sqlx::query("DELETE FROM matchmaking_queue WHERE pubkey = ?")
                        .bind(pubkey)
                        .execute(&state.pool)
                        .await
                    {
                        error!(
                            "[MATCHMAKING] Failed to clear persisted queue ticket: {}",
                            e
                        );
                    }
                }
                for (pubkey, opponent, is_white) in [
                    (&p1.pubkey, &p2.pubkey, true),
                    (&p2.pubkey, &p1.pubkey, false),
                ] {
                    if let Err(e) = sqlx::query(
                        "INSERT OR REPLACE INTO matchmaking_matches \
                         (pubkey, game_id, opponent, is_white, matched_at) VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(pubkey)
                    .bind(*game_id as i64)
                    .bind(opponent)
                    .bind(is_white)
                    .bind(now as i64)
                    .execute(&state.pool)
                    .await
                    {
                        error!("[MATCHMAKING] Failed to persist match: {}", e);
                    }
                }
            }
        }
    }
}
