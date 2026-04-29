//! Instruction to close a tournament and auto-distribute prizes to top 10.
//! Only callable after tournament completion.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
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
    /// CHECK: Prize escrow vault (85% of fees).
    #[account(
        mut,
        seeds = [TOURNAMENT_PRIZE_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub prize_escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Platform treasury vault for fee reimbursement
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: Authority (tournament host or admin)
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.status == TournamentStatus::Completed ||
        tournament.status == TournamentStatus::Active,
        GameErrorCode::InvalidTournamentStatus
    );

    // Validate authority (tournament host or admin)
    require!(
        ctx.accounts.authority.key() == tournament.authority ||
        ctx.accounts.authority.key() == crate::constants::vps_authority::ID,
        GameErrorCode::UnauthorizedAccess
    );

    // Mark tournament as closed
    tournament.status = TournamentStatus::Closed;

    // Distribute prizes if any remain
    let prize_escrow_lamports = ctx.accounts.prize_escrow_pda.lamports();
    if prize_escrow_lamports > 0 {
        let num_players = tournament.num_registered_players as usize;
        let mut remaining_lamports = prize_escrow_lamports;
        let mut distributed = 0;

        // Calculate total shares
        let total_shares: u64 = tournament.prize_shares.iter().map(|&s| s as u64).sum();
        if total_shares > 0 {
            // Distribute based on prize shares to top finishers
            for i in 0..tournament.prize_shares.len().min(num_players) {
                if remaining_lamports == 0 { break; }
                let share = tournament.prize_shares[i] as u64;
                if share == 0 { continue; }

                let prize_amount = (prize_escrow_lamports * share) / total_shares;
                if prize_amount == 0 { continue; }

                // Get player account from remaining_accounts
                if let Some(player_account) = ctx.remaining_accounts.get(i) {
                    **player_account.lamports.borrow_mut() += prize_amount;
                    remaining_lamports -= prize_amount;
                    distributed += prize_amount;
                    msg!("Distributed prize {} to player {}", prize_amount, player_account.key());
                }
            }
        } else if tournament.winner_takes_all && num_players > 0 {
            // Winner takes all
            if let Some(winner_account) = ctx.remaining_accounts.first() {
                **winner_account.lamports.borrow_mut() += prize_escrow_lamports;
                remaining_lamports = 0;
                distributed = prize_escrow_lamports;
                msg!("Distributed full prize {} to winner {}", prize_escrow_lamports, winner_account.key());
            }
        }

        // Return any undistributed lamports to treasury
        if remaining_lamports > 0 {
            **ctx.accounts.treasury_vault.lamports.borrow_mut() += remaining_lamports;
            msg!("Returned undistributed {} to treasury", remaining_lamports);
        }

        // Drain escrow account
        **ctx.accounts.prize_escrow_pda.lamports.borrow_mut() = 0;
    }

    msg!("Tournament {} closed", tournament_id);
    Ok(())
}
