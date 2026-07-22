//! Instruction for the operator to lock a guaranteed SOL prize pool in escrow.
//!
//! Must be called before any player registers: the guaranteed amount is written
//! once into `tournament.prize_pool` and can never be increased or decreased
//! while the tournament lives. This keeps the prize provably independent of how
//! many players enter — entry fees are operator revenue, never prize money.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, amount: u64)]
pub struct FundSolPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Tournament escrow PDA — holds the guaranteed SOL prize.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Operator funding the guaranteed prize.
    #[account(mut)]
    pub operator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<FundSolPrize>, tournament_id: u64, amount: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(amount > 0, GameErrorCode::InvalidArgument);
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    // The guarantee is locked exactly once, before the first registration.
    require!(
        tournament.prize_pool == 0,
        GameErrorCode::PrizeAlreadyFunded
    );
    require!(
        tournament.num_registered_players == 0,
        GameErrorCode::PrizeAlreadyFunded
    );

    anchor_lang::system_program::transfer(
        CpiContext::new(
            System::id(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.operator.to_account_info(),
                to: ctx.accounts.escrow_pda.to_account_info(),
            },
        ),
        amount,
    )?;

    tournament.prize_pool = amount;

    msg!(
        "Guaranteed SOL prize of {} lamports locked for tournament {}",
        amount,
        tournament_id
    );
    Ok(())
}
