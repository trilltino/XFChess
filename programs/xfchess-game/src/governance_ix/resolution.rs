//! Shared dispute resolution validation and state transitions.

use crate::errors::GameErrorCode;
use crate::state::{DisputeRecord, DisputeStatus, Game, GameResult, GameStatus};
use anchor_lang::prelude::*;

pub const MAX_DISPUTE_TEXT_LEN: usize = 200;

pub fn require_text_fits(text: &str) -> Result<()> {
    require!(
        text.len() <= MAX_DISPUTE_TEXT_LEN,
        GameErrorCode::InvalidArgument
    );
    Ok(())
}

pub fn validate_resolution(game: &Game, winner: Option<Pubkey>) -> Result<GameResult> {
    match winner {
        Some(winner_key) => {
            require!(
                winner_key == game.white || winner_key == game.black,
                GameErrorCode::InvalidWinner
            );
            Ok(GameResult::Winner(winner_key))
        }
        None => Ok(GameResult::Draw),
    }
}

pub fn apply_resolution(
    game: &mut Game,
    dispute: &mut DisputeRecord,
    result: GameResult,
    resolution: String,
    resolved_by: Pubkey,
    now: i64,
) -> Result<()> {
    require_text_fits(&resolution)?;
    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = resolution;
    dispute.resolved_at = Some(now);
    dispute.resolved_by = Some(resolved_by);
    game.result = result;
    game.status = GameStatus::Settled;
    game.updated_at = now;
    Ok(())
}
