//! Pure tournament-match validation helpers.

use crate::errors::GameErrorCode;
use crate::state::{Game, GameResult, TournamentMatch};
use anchor_lang::prelude::*;

/// Fails unless `winner`/`loser` are exactly the match's two stored
/// participants (in either order) and aren't the same player.
pub fn require_match_participants(
    tournament_match: &TournamentMatch,
    winner: Pubkey,
    loser: Pubkey,
) -> Result<()> {
    require!(winner != loser, GameErrorCode::InvalidWinner);
    let white = tournament_match
        .player_white
        .ok_or(GameErrorCode::MissingPlayerAccount)?;
    let black = tournament_match
        .player_black
        .ok_or(GameErrorCode::MissingPlayerAccount)?;
    require!(
        (winner == white && loser == black) || (winner == black && loser == white),
        GameErrorCode::InvalidWinner
    );
    Ok(())
}

/// Fails unless `game` is the tournament match's recorded game (when set),
/// `winner` is one of the game's two players, and the game's own result
/// agrees with `winner` — cross-checks the on-chain `Game` outcome before
/// recording a tournament match result from it.
pub fn require_game_matches_tournament_match(
    game: &Game,
    tournament_match: &TournamentMatch,
    game_key: Pubkey,
    winner: Pubkey,
) -> Result<()> {
    if let Some(expected_game) = tournament_match.game_pda {
        require_keys_eq!(expected_game, game_key, GameErrorCode::ResultMismatch);
    }
    require!(
        game.white == winner || game.black == winner,
        GameErrorCode::InvalidWinner
    );
    require!(
        game.result == GameResult::Winner(winner),
        GameErrorCode::ResultMismatch
    );
    Ok(())
}
