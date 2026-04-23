use crate::constants::COMMIT_ER_COST;
use crate::state::{Game, GameStatus};
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

pub fn commit_move_batch(
    ctx: Context<CommitMoveBatch>
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let fee_payer = &ctx.accounts.fee_payer;

    require!(game.status == GameStatus::Active, GameErrorCode::GameNotActive);
    require!(game.fee_payer == fee_payer.key(), GameErrorCode::FeePayerMismatch);

    game.fees_advanced = game.fees_advanced.checked_add(COMMIT_ER_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
    // Additional logic for committing move batch here

    Ok(())
}

#[derive(Accounts)]
pub struct CommitMoveBatch<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
