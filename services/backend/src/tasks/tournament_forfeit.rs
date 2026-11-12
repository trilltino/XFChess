use crate::signing::storage::tournament::{ResultSource, TournamentStatus};
use crate::signing::{AppState, OrchestratorEvent};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const FORFEIT_TICK: Duration = Duration::from_secs(10);

const FORFEIT_GRACE: Duration = Duration::from_secs(180);

pub fn spawn_tournament_forfeit_watcher(state: Arc<AppState>) {
    tokio::spawn(async move {
        info!(
            "[tournament-forfeit] auto-forfeit watcher started ({}s interval, {}s grace)",
            FORFEIT_TICK.as_secs(),
            FORFEIT_GRACE.as_secs()
        );
        let mut ticker = tokio::time::interval(FORFEIT_TICK);
        ticker.tick().await; // skip the immediate first tick

        // Track first-offline time per tournament/player. Rebuild each tick so
        // reconnected players and resolved matches drop out.
        let mut offline_since: HashMap<(u64, String), Instant> = HashMap::new();

        loop {
            ticker.tick().await;
            offline_since = run_tick(&state, offline_since).await;
        }
    });
}

async fn run_tick(
    state: &Arc<AppState>,
    offline_since: HashMap<(u64, String), Instant>,
) -> HashMap<(u64, String), Instant> {
    let online_node_ids: HashSet<String> = state
        .presence
        .get_all_online()
        .into_iter()
        .map(|p| p.node_id)
        .collect();

    let tournaments = state
        .tournament_store
        .with_status(TournamentStatus::Active)
        .await;
    let mut still_offline: HashMap<(u64, String), Instant> = HashMap::new();

    for t in tournaments {
        if let Some(bracket) = t.elimination() {
            let Some(round) = t.current_round() else {
                continue;
            };
            for m in &bracket.matches {
                if m.round != round || m.is_decided() {
                    continue;
                }
                let (Some(white), Some(black)) = (&m.player_white, &m.player_black) else {
                    continue; // opponent not decided yet — nothing to forfeit
                };

                for (player_id, opponent) in [(white, black), (black, white)] {
                    let Some(node_id) = t.node_ids.get(player_id) else {
                        continue;
                    };
                    if online_node_ids.contains(node_id) {
                        continue;
                    }

                    let key = (t.tournament_id, player_id.clone());
                    let since = offline_since
                        .get(&key)
                        .copied()
                        .unwrap_or_else(Instant::now);

                    if since.elapsed() < FORFEIT_GRACE {
                        still_offline.insert(key, since);
                        continue;
                    }

                    // Forfeit one player per tick when both are offline, avoiding immediate
                    // advancement of a second absent player.
                    info!(
                        "[tournament-forfeit] auto-forfeiting {} in single-elim tournament {} \
                         match {} round {} (offline {}s) — awarding to {}",
                        player_id,
                        t.tournament_id,
                        m.match_index,
                        round,
                        since.elapsed().as_secs(),
                        opponent
                    );
                    forfeit_bracket_match(
                        state,
                        t.tournament_id,
                        m.match_index,
                        opponent,
                        player_id,
                    )
                    .await;
                    break; // this match is resolved; don't also forfeit the opponent
                }
            }
            continue;
        }

        let Some(sd) = t.swiss() else {
            continue;
        };
        let round = sd.current_round;
        let Some(r) = sd.round(round) else {
            continue;
        };

        for pairing in &r.pairings {
            if sd.has_result(round, pairing.board) {
                continue;
            }

            for player_id in [&pairing.white, &pairing.black] {
                if sd.absent_players.contains(player_id) || sd.withdrawn_players.contains(player_id)
                {
                    continue;
                }

                let Some(node_id) = t.node_ids.get(player_id) else {
                    continue;
                };
                if online_node_ids.contains(node_id) {
                    continue; // seen this tick — no tracked offline-since carried forward
                }

                let key = (t.tournament_id, player_id.clone());
                let since = offline_since
                    .get(&key)
                    .copied()
                    .unwrap_or_else(Instant::now);

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
                    } else {
                        reconcile(state, t.tournament_id).await;
                    }
                    // Successful pairings are excluded by has_result/absent_players and need no retry.
                } else {
                    still_offline.insert(key, since);
                }
            }
        }
    }

    still_offline
}

/// Decides the match off-chain; the orchestrator mirrors it on-chain.
async fn forfeit_bracket_match(
    state: &Arc<AppState>,
    tournament_id: u64,
    match_index: u16,
    winner: &str,
    loser: &str,
) {
    match state
        .elimination_service
        .record_decision(
            tournament_id,
            match_index,
            winner,
            loser,
            ResultSource::Forfeit,
        )
        .await
    {
        Ok(_) => reconcile(state, tournament_id).await,
        Err(e) => warn!(
            "[tournament-forfeit] forfeit of match {match_index} in tournament {tournament_id} \
             not recorded: {e}"
        ),
    }
}

async fn reconcile(state: &Arc<AppState>, tournament_id: u64) {
    if let Some(tx) = &state.orchestrator_tx {
        if let Err(e) = tx
            .send(OrchestratorEvent::Reconcile { tournament_id })
            .await
        {
            warn!("[tournament-forfeit] orchestrator unavailable: {e}");
        }
    }
}
