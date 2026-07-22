//! Permissionless crank that pushes SOL prize shares to every recorded winner
//! in a single transaction, so winners receive funds without signing a claim.
//!
//! Destinations are constrained to the place pubkeys recorded on the
//! `Tournament` account, so the cranker cannot redirect funds. USDC pools stay
//! on the pull-based `claim_tournament_prize` path because recipient ATAs may
//! not exist at distribution time.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::prizes::ledger;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct DistributeTournamentPrizes<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: SOL escrow PDA — program-owned `TournamentEscrow`, so lamports
    /// can be debited directly. Seeds make it impossible to substitute.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Anyone may crank; payouts only ever go to recorded winners.
    pub cranker: Signer<'info>,
}

/// Pays each unclaimed place its SOL share, marking the claim bits used by
/// `claim_tournament_prize` so the two paths cannot double-pay.
///
/// `remaining_accounts` must contain the winners' (writable) wallet accounts in
/// any order; places whose wallet is absent are skipped and remain claimable.
/// Idempotent: re-cranking after full distribution is a no-op.
pub fn handler<'info>(
    ctx: Context<'info, DistributeTournamentPrizes<'info>>,
    _tournament_id: u64,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );
    require!(tournament.prize_pool > 0, GameErrorCode::NoPrizeToClaim);
    // Vesting payouts need per-claim schedule math — keep those pull-based.
    require!(
        tournament.payout_type == PayoutType::LumpSum,
        GameErrorCode::NoPrizeToClaim
    );

    let places = ledger::places(tournament);

    // The escrow account must stay rent-exempt until close_tournament reclaims it.
    let rent_min = Rent::get()?.minimum_balance(ctx.accounts.escrow_pda.data_len());

    let mut paid = 0u8;
    for (i, place) in places.iter().enumerate() {
        let Some(winner_key) = place else { continue };
        let share_bps = tournament.prize_shares[i];
        if share_bps == 0 {
            continue;
        }
        let place_bit = ledger::place_bit(i)?;
        if tournament.prizes_claimed & place_bit != 0 {
            continue;
        }
        let Some(wallet) = ctx
            .remaining_accounts
            .iter()
            .find(|a| a.key() == *winner_key)
        else {
            continue;
        };

        let prize = ledger::prize_amount(tournament.prize_pool, share_bps)?;
        if prize == 0 {
            continue;
        }

        let escrow_lamports = ctx.accounts.escrow_pda.lamports();
        require!(
            escrow_lamports.saturating_sub(prize) >= rent_min,
            GameErrorCode::InsufficientPrizeFunds
        );

        **ctx.accounts.escrow_pda.try_borrow_mut_lamports()? -= prize;
        **wallet.try_borrow_mut_lamports()? += prize;

        tournament.prizes_claimed |= place_bit;
        paid += 1;
    }

    msg!("distribute_tournament_prizes: paid {} place(s)", paid);
    Ok(())
}
