//! Instruction allowing the tournament operator to withdraw their escrow portion.
//! Only callable after tournament completion.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct OperatorWithdraw<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Operator escrow vault (5% of fees).
    #[account(
        mut,
        seeds = [TOURNAMENT_OPERATOR_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub operator_escrow_pda: UncheckedAccount<'info>,
    /// CHECK: The operator wallet receiving the funds.
    #[account(mut)]
    pub operator_wallet: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<OperatorWithdraw>, tournament_id: u64) -> Result<()> {
    let tournament = &ctx.accounts.tournament;

    // Only allow withdrawal after tournament completion
    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );

    // Get the operator escrow balance
    let escrow_balance = ctx.accounts.operator_escrow_pda.lamports();

    require!(escrow_balance > 0, GameErrorCode::NoPrizeToClaim);

    // Transfer from operator escrow to operator wallet
    **ctx.accounts.operator_escrow_pda.to_account_info().try_borrow_mut_lamports()? -= escrow_balance;
    **ctx.accounts.operator_wallet.to_account_info().try_borrow_mut_lamports()? += escrow_balance;

    msg!(
        "Operator withdrew {} lamports from tournament {} operator escrow",
        escrow_balance,
        tournament_id
    );

    Ok(())
}
