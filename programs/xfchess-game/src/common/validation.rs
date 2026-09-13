use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// Validates that a Tournament account's tournament_id field matches the expected value.
pub fn validate_tournament_id(
    tournament: &crate::state::Tournament,
    expected_id: u64,
) -> Result<()> {
    require!(
        tournament.tournament_id == expected_id,
        GameErrorCode::UnauthorizedAccess
    );
    Ok(())
}

/// Validates that a Game account's game_id field matches the expected value.
pub fn validate_game_id(game: &crate::state::Game, expected_id: u64) -> Result<()> {
    require!(
        game.game_id == expected_id,
        GameErrorCode::UnauthorizedAccess
    );
    Ok(())
}

/// Validates that an account's owner matches the expected program ID.
pub fn validate_account_owner(account: &AccountInfo, expected_owner: &Pubkey) -> Result<()> {
    require!(
        account.owner == expected_owner,
        GameErrorCode::InvalidAccountOwner
    );
    Ok(())
}

/// Validates that an account's data length matches the expected size.
pub fn validate_account_size(account: &AccountInfo, expected_size: usize) -> Result<()> {
    require!(
        account.data_len() == expected_size,
        GameErrorCode::InvalidAccountOwner
    );
    Ok(())
}
