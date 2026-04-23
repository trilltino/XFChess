use crate::constants::UNDELEGATE_COST;
use crate::state::{Game, GameStatus};
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

pub fn undelegate(
    ctx: Context<Undelegate>
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let fee_payer = &ctx.accounts.fee_payer;

    require!(game.status == GameStatus::Active || game.status == GameStatus::Finished, GameErrorCode::GameNotActive);
    require!(game.fee_payer == fee_payer.key(), GameErrorCode::FeePayerMismatch);

    game.fees_advanced = game.fees_advanced.checked_add(UNDELEGATE_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
    // Additional undelegation logic here

    Ok(())
}

#[derive(Accounts)]
pub struct Undelegate<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
