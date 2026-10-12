//! Creates the on-chain Game for every tournament match the store has assigned a
//! game id to, single elimination and Swiss alike.
//!
//! `create_tournament_game` is signed by the tournament authority and seats both
//! players at once, with per-game move sessions pointing at the backend session key
//! for that game id. The same key is what `/game/delegate` and `record_move` sign
//! with, so once a game is provisioned the normal play path (delegate → ER moves →
//! settlement worker → tournament dispatch) works without any player signature.
//!
//! Runs on a short timer and on demand from the tournament session routes, so a
//! player who reaches `MatchReady` finds their Game on-chain.

use crate::signing::solana::{create_tournament_game_ix, make_rpc, sign_and_submit, GAME_SEED};
pub use crate::signing::storage::tournament::AssignedGame;
use crate::signing::storage::tournament::{TournamentRecord, TournamentStatus};
use crate::signing::AppState;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const PROVISION_TICK: Duration = Duration::from_secs(5);

/// Matches of `t` that are assigned and undecided, minus those already provisioned.
pub fn games_to_provision(t: &TournamentRecord) -> Vec<AssignedGame> {
    if t.status != TournamentStatus::Active {
        return Vec::new();
    }
    t.state
        .open_games(t.tournament_id)
        .into_iter()
        .filter(|g| !t.provisioned_game_ids.contains(&g.game_id))
        .collect()
}

pub fn spawn_tournament_game_provisioner(state: Arc<AppState>) {
    tokio::spawn(async move {
        info!(
            "[tournament-games] provisioner started ({}s interval)",
            PROVISION_TICK.as_secs()
        );
        let mut ticker = tokio::time::interval(PROVISION_TICK);
        loop {
            ticker.tick().await;
            let running = state
                .tournament_store
                .with_status(TournamentStatus::Active)
                .await;
            for t in running {
                for game in games_to_provision(&t) {
                    if let Err(e) = provision(&state, &game, t.max_players).await {
                        warn!(
                            "[tournament-games] tournament {} game {}: {}",
                            game.tournament_id, game.game_id, e
                        );
                        break; // retry this tournament next tick
                    }
                }
            }
        }
    });
}

/// Provisions `game_id` now if it is one of `tournament_id`'s assigned, unprovisioned
/// games. Called by the session routes so a player never waits a full tick.
pub async fn provision_now(
    state: &AppState,
    tournament_id: u64,
    game_id: u64,
) -> Result<(), String> {
    let Some(t) = state.tournament_store.get(tournament_id).await else {
        return Err(format!("tournament {tournament_id} not found"));
    };
    if t.provisioned_game_ids.contains(&game_id) {
        return Ok(());
    }
    let Some(game) = games_to_provision(&t)
        .into_iter()
        .find(|g| g.game_id == game_id)
    else {
        return Err(format!("game {game_id} is not an assigned match"));
    };
    provision(state, &game, t.max_players).await
}

async fn provision(state: &AppState, game: &AssignedGame, max_players: u16) -> Result<(), String> {
    let white = Pubkey::from_str(&game.white).map_err(|e| format!("white pubkey: {e}"))?;
    let black = Pubkey::from_str(&game.black).map_err(|e| format!("black pubkey: {e}"))?;

    // Get-or-create the game's backend session key and mark it live: the
    // settlement worker only watches active sessions.
    let session_key = state
        .store
        .create(game.game_id, white)
        .await
        .map_err(|e| format!("session key: {e}"))?;
    state.store.activate(game.game_id).await;

    let rpc_url = state.solana_rpc_url.clone();
    let program_id = state.program_id;
    let authority = state.vps_authority.clone();
    let (tournament_id, game_id) = (game.tournament_id, game.game_id);
    let created = tokio::task::spawn_blocking(move || {
        let rpc = make_rpc(&rpc_url);
        let game_pda =
            Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
        if rpc.get_account(&game_pda).is_ok() {
            return Ok(false);
        }
        let ix = create_tournament_game_ix(
            &program_id,
            tournament_id,
            max_players,
            game_id,
            &white,
            &black,
            &session_key,
            &authority.pubkey(),
        );
        match sign_and_submit(&rpc, &authority, &[ix]) {
            Ok(_) => Ok(true),
            // A concurrent provision (timer vs. session route) got there first.
            Err(_) if rpc.get_account(&game_pda).is_ok() => Ok(false),
            Err(e) => Err(format!("create_tournament_game: {e}")),
        }
    })
    .await
    .map_err(|e| format!("provision task panicked: {e}"))??;

    if !state
        .tournament_store
        .update(tournament_id, |t| {
            if !t.provisioned_game_ids.contains(&game_id) {
                t.provisioned_game_ids.push(game_id);
            }
        })
        .await
    {
        error!("[tournament-games] tournament {tournament_id} vanished while provisioning game {game_id}");
    }
    if created {
        info!(
            "[tournament-games] tournament {} game {} created on-chain: {} (W) vs {} (B)",
            tournament_id, game_id, game.white, game.black
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::storage::tournament::{
        EliminationState, FormatState, GameOutcome, SwissStorageData, TournamentStore,
    };

    fn active(state: FormatState) -> TournamentRecord {
        let mut t = TournamentRecord::new(5, "t", 0);
        t.state = state;
        t.status = TournamentStatus::Active;
        t
    }

    #[test]
    fn bracket_games_are_the_seated_undecided_unprovisioned_matches() {
        let seeds: Vec<String> = (0..4).map(|i| format!("P{i}")).collect();
        let mut bracket = EliminationState::draw(5, &seeds);
        let (first, second) = (
            bracket.matches[0].game_id.unwrap(),
            bracket.matches[1].game_id.unwrap(),
        );
        bracket
            .apply_game(5, first, "P0", "P3", GameOutcome::WhiteWin, &seeds)
            .unwrap();
        let mut t = active(FormatState::Elimination(bracket));
        // Match 0 is decided; the final still waits for its second player.
        let ids: Vec<u64> = games_to_provision(&t).iter().map(|g| g.game_id).collect();
        assert_eq!(ids, vec![second]);

        t.provisioned_game_ids = vec![second];
        assert!(games_to_provision(&t).is_empty());
    }

    #[test]
    fn a_drawn_bracket_game_needs_its_replay_provisioned() {
        let seeds: Vec<String> = vec!["A".into(), "B".into()];
        let mut bracket = EliminationState::draw(5, &seeds);
        let first = bracket.matches[0].game_id.unwrap();
        bracket
            .apply_game(5, first, "A", "B", GameOutcome::Draw, &seeds)
            .unwrap();
        let mut t = active(FormatState::Elimination(bracket));
        t.provisioned_game_ids = vec![first];
        let games = games_to_provision(&t);
        assert_eq!(games.len(), 1);
        assert_ne!(games[0].game_id, first);
        assert_eq!(
            (games[0].white.as_str(), games[0].black.as_str()),
            ("B", "A")
        );
    }

    #[test]
    fn swiss_games_are_the_open_boards_of_the_latest_round() {
        let pairing = |board: u16| swiss_pairing::Pairing {
            white: format!("w{board}"),
            black: format!("b{board}"),
            board,
        };
        let mut sd = SwissStorageData::new(3);
        sd.current_round = 2;
        sd.rounds = vec![
            swiss_pairing::SwissRound {
                round: 1,
                pairings: vec![pairing(1)],
                byes: vec![],
                float_downs: vec![],
                float_ups: vec![],
            },
            swiss_pairing::SwissRound {
                round: 2,
                pairings: vec![pairing(1), pairing(2)],
                byes: vec![],
                float_downs: vec![],
                float_ups: vec![],
            },
        ];
        sd.results = vec![(2, 1, swiss_pairing::MatchResult::Draw)];
        let mut t = active(FormatState::Swiss(sd));
        let games = games_to_provision(&t);
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].game_id,
            TournamentStore::swiss_game_id(5, 2, 2).unwrap()
        );
        assert_eq!(
            (games[0].white.as_str(), games[0].black.as_str()),
            ("w2", "b2")
        );

        t.status = TournamentStatus::Completed;
        assert!(games_to_provision(&t).is_empty());
    }
}
