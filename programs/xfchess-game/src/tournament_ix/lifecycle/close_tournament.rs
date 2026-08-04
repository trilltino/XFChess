//! Instruction to close a tournament and auto-distribute prizes to top 10.
//! Only callable after tournament completion.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
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
    /// CHECK: Prize escrow vault (entry fees).
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub prize_escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Platform treasury vault for fee reimbursement
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// Tournament host or platform admin — must sign to authorize the close.
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );

    // Close only AFTER completion. Never during Active — results may not exist,
    // and flipping to Closed would disable the Completed-gated payout paths.
    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::InvalidTournamentStatus
    );

    // Validate authority (tournament host or platform admin).
    require!(
        ctx.accounts.authority.key() == tournament.authority
            || ctx.accounts.authority.key() == crate::constants::vps_authority::ID,
        GameErrorCode::UnauthorizedAccess
    );

    // Winners are paid exclusively via distribute_tournament_prizes (push,
    // winner-constrained) and claim_tournament_prize (pull). This instruction
    // NEVER pays arbitrary accounts. Before disabling those Completed-gated
    // paths, require every funded prize place to already be claimed so no
    // unpaid winner is stranded.
    for i in 0..ledger::MAX_PRIZE_PLACES {
        require!(
            !ledger::funded_place_unclaimed(tournament, i)?,
            GameErrorCode::PrizesOutstanding
        );
    }

    // Every winner is paid, so whatever remains in escrow (bps rounding
    // remainder + unallocated shares + the account's own rent) is operator
    // revenue. Sweep the full balance to the platform treasury and let the now
    // zero-lamport escrow account be reclaimed. The escrow is program-owned
    // (TournamentEscrow), so a direct lamport debit is correct.
    let escrow_ai = ctx.accounts.prize_escrow_pda.to_account_info();
    let sweep = escrow_ai.lamports();
    if sweep > 0 {
        **escrow_ai.try_borrow_mut_lamports()? -= sweep;
        **ctx
            .accounts
            .treasury_vault
            .to_account_info()
            .try_borrow_mut_lamports()? += sweep;
    }

    tournament.status = TournamentStatus::Closed;
    Ok(())
}
