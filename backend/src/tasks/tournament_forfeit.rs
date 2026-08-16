//! Automatic tournament forfeiture.
//!
//! `mark_absent`/`withdraw_player` (`signing/swiss/service.rs`) previously
//! only fired from a manual admin HTTP call (`POST /tournament/{id}/absent`)
//! — a seated player who simply vanishes mid-round (closed the app, lost
//! power, crashed) stayed "pending" forever unless a human noticed and acted.
//! This task closes that gap by reusing the same `mark_absent` call
//! automatically once a seated, unresolved pairing's player has been
//! continuously offline (per the presence store — see
//! `backend/src/signing/social/presence.rs`) for longer than
//! [`FORFEIT_GRACE`].

use crate::signing::storage::tournament::TournamentStatus;
use crate::signing::AppState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// How often to re-scan active tournaments' current-round pairings.
const FORFEIT_TICK: Duration = Duration::from_secs(10);

/// How long a seated player must be continuously offline before being
/// auto-forfeited. Comfortably above the presence store's own online/offline
/// cutoff (`ONLINE_FRESHNESS_SECS`) plus this task's own poll jitter, and
/// long enough that a brief reconnect blip doesn't cost someone their match.
const FORFEIT_GRACE: Duration = Duration::from_secs(45);

/// Spawns the background auto-forfeit watcher.
pub fn spawn_tournament_forfeit_watcher(state: Arc<AppState>) {
    tokio::spawn(async move {
        info!(
            "[tournament-forfeit] auto-forfeit watcher started ({}s interval, {}s grace)",
            FORFEIT_TICK.as_secs(),
            FORFEIT_GRACE.as_secs()
        );
        let mut ticker = tokio::time::interval(FORFEIT_TICK);
        ticker.tick().await; // skip the immediate first tick

        // (tournament_id, player wallet pubkey) -> first observed offline.
        // Owned by this task's loop — rebuilt fresh every tick from whoever
        // is *still* offline and unresolved, so a reconnect or a completed
        // pairing naturally drops out instead of needing explicit cleanup.
        let mut offline_since: HashMap<(u64, String), Instant> = HashMap::new();

        loop {
            ticker.tick().await;
            offline_since = run_tick(&state, offline_since).await;
        }
    });
}

async fn run_tick(
    state: &Arc<AppState>,
    mut offline_since: HashMap<(u64, String), Instant>,
) -> HashMap<(u64, String), Instant> {
    let online_node_ids: HashSet<String> = state
        .presence
        .get_all_online()
        .into_iter()
        .map(|p| p.node_id)
        .collect();

    let tournaments = state.tournament_store.list().await;
    let mut still_offline: HashMap<(u64, String), Instant> = HashMap::new();

    for t in tournaments {
        if t.status != TournamentStatus::Active {
            continue;
        }
        let Some(sd) = t.swiss_data.as_ref() else {
            continue;
        };
        let round = sd.current_round;
        let Some(r) = sd.rounds.iter().find(|r| r.round == round) else {
            continue;
        };

        for pairing in &r.pairings {
            let has_result = sd
                .results
                .iter()
                .any(|(rd, board, _)| *rd == round && *board == pairing.board);
            if has_result {
                continue;
            }

            for player_id in [&pairing.white, &pairing.black] {
                if sd.absent_players.contains(player_id) || sd.withdrawn_players.contains(player_id)
                {
                    continue;
                }

                let is_online = t
                    .node_ids
                    .get(player_id)
                    .map(|node_id| online_node_ids.contains(node_id))
                    .unwrap_or(false);
                if is_online {
                    continue; // seen this tick — no tracked offline-since carried forward
                }

                let key = (t.tournament_id, player_id.clone());
                let since = offline_since.get(&key).copied().unwrap_or_else(Instant::now);

                if since.elapsed() >= FORFEIT_GRACE {
                    info!(
                        "[tournament-forfeit] auto-forfeiting {} in tournament {} round {} (offline {}s)",
                        player_id,
                        t.tournament_id,
                        round,
                        since.elapsed().as_secs()
                    );
                    if let Err(e) = state
                        .swiss_service
                        .mark_absent(t.tournament_id, player_id, round)
                        .await
                    {
                        warn!(
                            "[tournament-forfeit] mark_absent failed for {} in tournament {}: {e:?}",
                            player_id, t.tournament_id
                        );
                        still_offline.insert(key, since); // retry next tick
                    }
                    // On success, deliberately not re-tracked: `has_result`/
                    // `absent_players` above will exclude this pairing from
                    // now on, so there's nothing left to retry.
                } else {
                    still_offline.insert(key, since);
                }
            }
        }
    }

    still_offline
}
