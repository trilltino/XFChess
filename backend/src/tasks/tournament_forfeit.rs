use crate::signing::solana::{record_result_ix, sign_and_submit};
use crate::signing::storage::tournament::{MatchStatus, TournamentFormat, TournamentStatus};
use crate::signing::AppState;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const FORFEIT_TICK: Duration = Duration::from_secs(10);

const FORFEIT_GRACE: Duration = Duration::from_secs(45);

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
    offline_since: HashMap<(u64, String), Instant>,
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

        // ── Single-elimination ────────────────────────────────────────────
        if t.format == TournamentFormat::SingleElimination {
            let Some(round) = t.current_round() else {
                continue;
            };
            for m in t.matches.iter().flatten() {
                if m.round != round || m.status == MatchStatus::Completed {
                    continue;
                }
                let (Some(white), Some(black)) = (m.player_white.clone(), m.player_black.clone())
                else {
                    continue; // opponent not decided yet — nothing to forfeit
                };

                for (player_id, opponent) in [(&white, &black), (&black, &white)] {
                    let is_online = t
                        .node_ids
                        .get(player_id)
                        .map(|node_id| online_node_ids.contains(node_id))
                        .unwrap_or(false);
                    if is_online {
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

                    // Both offline? Forfeit only one per tick — awarding the
                    // match to a player who is also gone would advance a
                    // no-show and stall the *next* round instead. The
                    // opponent gets the same treatment on a later tick if
                    // they're still absent when they're due to play.
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
                    forfeit_single_elim_match(
                        state,
                        t.tournament_id,
                        m.match_index as usize,
                        opponent,
                        player_id,
                    )
                    .await;
                    break; // this match is resolved; don't also forfeit the opponent
                }
            }
            continue;
        }

        // ── Swiss ─────────────────────────────────────────────────────────
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

async fn forfeit_single_elim_match(
    state: &Arc<AppState>,
    tournament_id: u64,
    match_index: usize,
    winner: &str,
    loser: &str,
) {
    let store = &state.tournament_store;
    if !store
        .record_result(
            tournament_id,
            match_index,
            winner.to_string(),
            loser.to_string(),
        )
        .await
    {
        warn!(
            "[tournament-forfeit] record_result failed for match {} of tournament {}",
            match_index, tournament_id
        );
        return;
    }

    let (Ok(program_id), Ok(winner_pk), Ok(loser_pk)) = (
        Pubkey::from_str(&state.config.program_id),
        Pubkey::from_str(winner),
        Pubkey::from_str(loser),
    ) else {
        // Bot/test wallets aren't real pubkeys; the store result still stands.
        return;
    };

    let authority = state.vps_authority.clone();
    let rpc_url = state.config.solana_rpc_url.clone();
    let next = store.get(tournament_id).await.and_then(|t| {
        t.matches
            .get(match_index)
            .and_then(|m| m.as_ref())
            .and_then(|m| m.next_match_for_winner)
    });

    let submitted = tokio::task::spawn_blocking(move || {
        let rpc = crate::signing::solana::make_rpc(&rpc_url);
        let ix = record_result_ix(
            &program_id,
            tournament_id,
            match_index as u16,
            &winner_pk,
            &loser_pk,
            &authority.pubkey(),
        );
        sign_and_submit(&rpc, &authority, &[ix]).map_err(|e| e.to_string())?;
        if let Some(next_idx) = next {
            let ix = crate::signing::solana::advance_winner_ix(
                &program_id,
                tournament_id,
                match_index as u16,
                next_idx,
                &authority.pubkey(),
            );
            sign_and_submit(&rpc, &authority, &[ix]).map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await;

    match submitted {
        Ok(Ok(())) => {}
        Ok(Err(e)) => error!(
            "[tournament-forfeit] on-chain mirror failed for match {} of tournament {}: {e}",
            match_index, tournament_id
        ),
        Err(e) => error!("[tournament-forfeit] on-chain mirror task panicked: {e}"),
    }
}
