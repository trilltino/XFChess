//! Instruction to initialize the tournament escrow PDA.
//! Must be called after `initialize_tournament` and before `register_player`.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

/// Minimal account type for the tournament SOL escrow (no data, just holds lamports).
#[account]
pub struct TournamentEscrow {}

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct InitializeTournamentEscrow<'info> {
    #[account(
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,

    #[account(
        init,
        payer = authority,
        space = 8,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Account<'info, TournamentEscrow>,

    #[account(
        mut,
        constraint = authority.key() == vps_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    _ctx: Context<InitializeTournamentEscrow>,
    _tournament_id: u64,
) -> Result<()> {
    msg!("Tournament escrow initialized");
    Ok(())
}
