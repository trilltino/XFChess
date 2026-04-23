use crate::constants::{MARGIN_BPS};
use crate::state::{Game, GameStatus};
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;
use anchor_lang::system_program;

pub fn claim_prize(
    ctx: Context<ClaimPrize>,
) -> Result<()> {
    let game = &ctx.accounts.game;
    let escrow = &mut ctx.accounts.escrow;
    let winner = &mut ctx.accounts.winner;
    let fee_payer = &mut ctx.accounts.fee_payer;

    // Ensure game is finished
    require!(game.status == GameStatus::Finished, GameErrorCode::GameNotFinished);

    // Calculate wager pool and rebate
    let wager_pool = game.wager_amount.checked_mul(2).ok_or(GameErrorCode::ArithmeticOverflow)?;
    let margin = wager_pool.checked_mul(MARGIN_BPS as u64).ok_or(GameErrorCode::ArithmeticOverflow)?.checked_div(10_000).ok_or(GameErrorCode::ArithmeticOverflow)?;
    let rebate = game.fees_advanced.checked_add(margin).ok_or(GameErrorCode::ArithmeticOverflow)?;

    // Ensure pool can cover fees and margin
    require!(wager_pool >= rebate, GameErrorCode::PoolTooSmallForFees);
    let prize = wager_pool.checked_sub(rebate).ok_or(GameErrorCode::ArithmeticOverflow)?;

    // Transfer rebate to fee_payer (relayer)
    **escrow.try_borrow_mut_lamports()? -= rebate;
    **fee_payer.try_borrow_mut_lamports()? += rebate;

    // Transfer prize to winner
    **escrow.try_borrow_mut_lamports()? -= prize;
    **winner.try_borrow_mut_lamports()? += prize;

    // Close escrow account, rent reclaim to fee_payer
    system_program::close_account(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::CloseAccount {
                account: escrow.to_account_info(),
                destination: fee_payer.to_account_info(),
                authority: fee_payer.to_account_info(),
            },
        ),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub escrow: SystemAccount<'info>,
    #[account(mut)]
    pub winner: Signer<'info>,
    #[account(mut)]
    pub fee_payer: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
