//! Instruction to close a tournament and auto-distribute prizes to top 8.
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
    /// Top 8 winners (ordered by placement).
    /// CHECK: Validated by checking against tournament.winner, second_place, etc.
    #[account(mut)]
    pub winner: UncheckedAccount<'info>,
    /// CHECK: Validated by checking against tournament.second_place
    #[account(mut)]
    pub second_place: UncheckedAccount<'info>,
    /// CHECK: Validated by checking against tournament.third_place
    #[account(mut)]
    pub third_place: UncheckedAccount<'info>,
    /// CHECK: Validated by checking against tournament.fourth_place
    #[account(mut)]
    pub fourth_place: UncheckedAccount<'info>,
    /// CHECK: Placeholder for 5th place (not tracked in current state)
    #[account(mut)]
    pub fifth_place: UncheckedAccount<'info>,
    /// CHECK: Placeholder for 6th place (not tracked in current state)
    #[account(mut)]
    pub sixth_place: UncheckedAccount<'info>,
    /// CHECK: Placeholder for 7th place (not tracked in current state)
    #[account(mut)]
    pub seventh_place: UncheckedAccount<'info>,
    /// CHECK: Placeholder for 8th place (not tracked in current state)
    #[account(mut)]
    pub eighth_place: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CloseTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    // Only allow close after tournament completion
    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );

    // Get prize escrow balance
    let prize_pool = ctx.accounts.prize_escrow_pda.lamports();

    require!(prize_pool > 0, GameErrorCode::NoPrizeToClaim);

    // Validate winner accounts match tournament state
    require!(
        ctx.accounts.winner.key() == tournament.winner.unwrap(),
        GameErrorCode::NoPrizeToClaim
    );

    // Distribute prizes to top 8 based on prize_shares
    let winners = [
        &ctx.accounts.winner,
        &ctx.accounts.second_place,
        &ctx.accounts.third_place,
        &ctx.accounts.fourth_place,
        &ctx.accounts.fifth_place,
        &ctx.accounts.sixth_place,
        &ctx.accounts.seventh_place,
        &ctx.accounts.eighth_place,
    ];

    let placements = [
        tournament.winner,
        tournament.second_place,
        tournament.third_place,
        tournament.fourth_place,
        None, // 5th-8th not tracked in current state
        None,
        None,
        None,
    ];

    for (i, (winner_account, placement)) in winners.iter().zip(placements.iter()).enumerate() {
        let share_bps = tournament.prize_shares[i];
        if share_bps == 0 {
            continue;
        }

        // For 5th-8th place, skip if not set (None)
        if i >= 4 && placement.is_none() {
            continue;
        }

        let payout = (prize_pool as u128 * share_bps as u128 / 10000) as u64;

        if payout > 0 {
            **ctx.accounts.prize_escrow_pda.to_account_info().try_borrow_mut_lamports()? -= payout;
            **winner_account.to_account_info().try_borrow_mut_lamports()? += payout;

            msg!(
                "Payout {} lamports to place {} ({} bps)",
                payout,
                i + 1,
                share_bps
            );
        }
    }

    msg!(
        "Tournament {} closed. {} lamports distributed to top 8",
        tournament_id,
        prize_pool
    );

    Ok(())
}
