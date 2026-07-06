//! Terminal result transitions that mutate only the Game account.

use crate::errors::GameErrorCode;
use crate::lifecycle::clock;
use crate::state::{Game, GameResult, GameStatus};
use anchor_lang::prelude::*;

pub fn finish_by_resign(game: &mut Game, resigner: Pubkey, now: i64) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        resigner == game.white || resigner == game.black,
        GameErrorCode::NotInGame
    );

    let winner = if resigner == game.white {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    clock::mark_terminal(game, now);
    Ok(())
}

pub fn finish_by_timeout(game: &mut Game, now: i64) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    let inactivity_window = clock::inactivity_window_seconds(game);
    require!(
        now - game.updated_at > inactivity_window,
        GameErrorCode::TimeoutNotExpired
    );

    let white_timed_out = game.turn % 2 == 1;
    let winner = if white_timed_out {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    clock::mark_terminal(game, now);
    Ok(())
}

pub fn finish_by_timeout_if_expired(game: &mut Game, now: i64) -> Result<bool> {
    if game.status != GameStatus::Active || game.base_time_seconds == 0 {
        return Ok(false);
    }

    let time_elapsed = (now - game.updated_at) as u64;
    let time_limit = game.base_time_seconds.saturating_mul(3);
    if time_elapsed <= time_limit {
        return Ok(false);
    }

    let white_timed_out = game.turn % 2 == 1;
    let winner = if white_timed_out {
        game.black
    } else {
        game.white
    };

    game.status = GameStatus::Finished;
    game.result = GameResult::Winner(winner);
    clock::mark_terminal(game, now);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_game() -> Game {
        Game {
            game_id: 1,
            white: Pubkey::new_unique(),
            black: Pubkey::new_unique(),
            status: GameStatus::Active,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 0,
            wager_token: None,
            game_type: crate::state::GameType::PvP,
            match_type: crate::state::MatchType::Free,
            country_fee: 0,
            base_time_seconds: 1,
            increment_seconds: 0,
            bump: 0,
            is_delegated: true,
            tournament_id: None,
            nonce: 0,
        }
    }

    #[test]
    fn resign_finishes_without_clearing_delegation() {
        let mut game = active_game();
        let white = game.white;
        let black = game.black;

        finish_by_resign(&mut game, white, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Winner(black));
        assert!(game.is_delegated);
        assert_eq!(game.updated_at, 10);
    }

    #[test]
    fn timeout_finishes_without_clearing_delegation() {
        let mut game = active_game();
        let black = game.black;

        finish_by_timeout(&mut game, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Winner(black));
        assert!(game.is_delegated);
    }
}
