use crate::common::validation;
use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
use crate::tournament_ix::prizes::ledger;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct CloseTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump,
        close = treasury_vault
    )]
    pub prize_escrow_pda: Account<'info, TournamentEscrow>,
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    validation::validate_tournament_id(tournament, tournament_id)?;

    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::InvalidTournamentStatus
    );

    require!(
        ctx.accounts.authority.key() == tournament.authority
            || ctx.accounts.authority.key() == crate::constants::vps_authority::ID,
        GameErrorCode::UnauthorizedAccess
    );

    for i in 0..ledger::MAX_PRIZE_PLACES {
        require!(
            !ledger::funded_place_unclaimed(tournament, i)?,
            GameErrorCode::PrizesOutstanding
        );
    }

    tournament.status = TournamentStatus::Closed;
    Ok(())
}
