use crate::constants::RECORD_RESULT_COST;
use crate::state::{Game, GameStatus};
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

pub fn record_result(
    ctx: Context<RecordResult>
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let fee_payer = &ctx.accounts.fee_payer;

    require!(game.status == GameStatus::Active, GameErrorCode::GameNotActive);
    require!(game.fee_payer == fee_payer.key(), GameErrorCode::FeePayerMismatch);

    game.status = GameStatus::Finished;
    game.fees_advanced = game.fees_advanced.checked_add(RECORD_RESULT_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
    // Additional logic for recording game result here

    Ok(())
}

#[derive(Accounts)]
pub struct RecordResult<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
