//! Small lifecycle transition helpers used by instruction adapters.

use crate::constants::{DELEGATE_COST, JOIN_GAME_COST};
use crate::errors::GameErrorCode;
use crate::state::{Game, GameStatus};
use anchor_lang::prelude::*;

pub fn join_waiting_game(
    game: &mut Game,
    joiner: Pubkey,
    fee_payer: Pubkey,
    now: i64,
) -> Result<()> {
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(game.white != joiner, GameErrorCode::CannotPlaySelf);
    require!(game.fee_payer == fee_payer, GameErrorCode::FeePayerMismatch);

    game.black = joiner;
    game.status = GameStatus::Active;
    game.fees_advanced = game
        .fees_advanced
        .checked_add(JOIN_GAME_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.last_move_timestamp = now;
    game.updated_at = now;
    Ok(())
}

pub fn mark_delegated(game: &mut Game) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(!game.is_delegated, GameErrorCode::GameAlreadyDelegated);
    game.fees_advanced = game
        .fees_advanced
        .checked_add(DELEGATE_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.is_delegated = true;
    Ok(())
}

pub fn mark_undelegated(game: &mut Game) -> Result<()> {
    require!(game.is_delegated, GameErrorCode::GameNotDelegated);
    game.is_delegated = false;
    Ok(())
}
