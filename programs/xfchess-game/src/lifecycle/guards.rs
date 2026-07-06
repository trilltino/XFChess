//! Reusable lifecycle precondition checks.

use crate::errors::GameErrorCode;
use crate::state::{Game, GamePhase};
use anchor_lang::prelude::*;

pub fn require_undelegated(game: &Game) -> Result<()> {
    require!(
        !game.is_delegated,
        GameErrorCode::SettlementRequiresUndelegated
    );
    Ok(())
}

pub fn require_delegated(game: &Game) -> Result<()> {
    require!(game.is_delegated, GameErrorCode::GameNotDelegated);
    Ok(())
}

pub fn require_phase(game: &Game, expected: GamePhase) -> Result<()> {
    require!(
        game.phase()? == expected,
        GameErrorCode::InvalidLifecycleTransition
    );
    Ok(())
}
