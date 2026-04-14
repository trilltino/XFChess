//! Instruction allowing winners to claim their tournament prize shares.
//! Supports top-4 distribution: 1st, 2nd, 3rd, and 4th place.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct ClaimTournamentPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Escrow PDA holding prize pool.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Claimant's wallet — must match a winning position.
    #[account(mut, constraint = claimant_wallet.key() == claimant.key() @ GameErrorCode::UnauthorizedAccess)]
    pub claimant_wallet: UncheckedAccount<'info>,
    pub claimant: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimTournamentPrize>, tournament_id: u64) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    let claimant_key = ctx.accounts.claimant.key();

    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );

    // Determine which place the claimant finished and their prize share
    let (place, prize_share_bps) = if Some(claimant_key) == tournament.winner {
        (1u8, tournament.prize_shares[0])
    } else if Some(claimant_key) == tournament.second_place {
        (2u8, tournament.prize_shares[1])
    } else if Some(claimant_key) == tournament.third_place {
        (3u8, tournament.prize_shares[2])
    } else if Some(claimant_key) == tournament.fourth_place {
        (4u8, tournament.prize_shares[3])
    } else {
        return Err(GameErrorCode::UnauthorizedAccess.into());
    };

    require!(prize_share_bps > 0, GameErrorCode::NoPrizeToClaim);

    // Calculate prize amount (prize_share in basis points / 10000 * prize_pool)
    let prize = (tournament.prize_pool as u128)
        .checked_mul(prize_share_bps as u128)
        .unwrap()
        .checked_div(10000)
        .unwrap() as u64;

    require!(prize > 0, GameErrorCode::NoPrizeToClaim);

    let tournament_id_bytes = tournament_id.to_le_bytes();
    let bump = ctx.bumps.escrow_pda;
    let escrow_seeds: &[&[&[u8]]] = &[&[TOURNAMENT_ESCROW_SEED, &tournament_id_bytes, &[bump]]];

    anchor_lang::system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.escrow_pda.to_account_info(),
                to: ctx.accounts.claimant_wallet.to_account_info(),
            },
            escrow_seeds,
        ),
        prize,
    )?;

    msg!(
        "Tournament {} prize claimed: {} lamports to {} (Place {} - {}%)",
        tournament_id,
        prize,
        claimant_key,
        place,
        prize_share_bps as f64 / 100.0
    );
    Ok(())
}
