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
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(MATCHMAKING_INTERVAL_SECONDS));

    loop {
        interval.tick().await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut queue = match state.queue.lock() {
            Ok(q) => q,
            Err(e) => {
                error!("[MATCHMAKING] Queue mutex poisoned: {}", e);
                continue;
            }
        };
        if queue.len() < MATCHMAKING_MIN_PLAYERS {
            continue;
        }

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

        // Save results to match map.
        if !new_matches.is_empty() {
            let mut matches = match state.matches.lock() {
                Ok(m) => m,
                Err(e) => {
                    error!("[MATCHMAKING] Matches mutex poisoned: {}", e);
                    continue;
                }
            };
            for (p1, p2, game_id) in new_matches {
                matches.insert(
                    p1.pubkey.clone(),
                    MatchResult {
                        game_id,
                        opponent: p2.pubkey.clone(),
                        is_white: true,
                    },
                );
                matches.insert(
                    p2.pubkey.clone(),
                    MatchResult {
                        game_id,
                        opponent: p1.pubkey.clone(),
                        is_white: false,
                    },
                );
            }
        }
    }
}
