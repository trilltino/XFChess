use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
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
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Account<'info, TournamentEscrow>,
    pub cranker: Signer<'info>,
}

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
    require!(
        tournament.payout_type == PayoutType::LumpSum,
        GameErrorCode::NoPrizeToClaim
    );

    let places = ledger::places(tournament);
    let expected_wallets: Vec<Pubkey> = places
        .iter()
        .enumerate()
        .filter_map(|(i, place)| {
            if tournament.prize_shares[i] > 0
                && tournament.prizes_claimed & ledger::place_bit(i).ok()? == 0
            {
                *place
            } else {
                None
            }
        })
        .collect();
    require!(
        ctx.remaining_accounts.len() == expected_wallets.len(),
        GameErrorCode::InvalidRemainingAccounts
    );

    let mut seen_wallets = Vec::with_capacity(ctx.remaining_accounts.len());
    for (account, expected_key) in ctx.remaining_accounts.iter().zip(expected_wallets.iter()) {
        require_keys_eq!(
            account.key(),
            *expected_key,
            GameErrorCode::InvalidRemainingAccounts
        );
        require!(
            account.is_writable && account.owner == &system_program::ID,
            GameErrorCode::InvalidRemainingAccounts
        );
        require!(
            !seen_wallets.iter().any(|key| key == expected_key),
            GameErrorCode::InvalidRemainingAccounts
        );
        seen_wallets.push(*expected_key);
    }

    let mut paid = 0usize;
    let mut wallet_index = 0usize;
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
        let wallet = &ctx.remaining_accounts[wallet_index];
        wallet_index += 1;
        require_keys_eq!(
            wallet.key(),
            *winner_key,
            GameErrorCode::InvalidRemainingAccounts
        );

        let prize = ledger::prize_amount(tournament.prize_pool, share_bps)?;
        if prize == 0 {
            continue;
        }

        crate::common::escrow::debit_program_pda(
            &ctx.accounts.escrow_pda.to_account_info(),
            wallet,
            prize,
        )
        .map_err(|_| GameErrorCode::InsufficientPrizeFunds)?;

        tournament.prizes_claimed |= place_bit;
        paid += 1;
    }

    msg!("distribute_tournament_prizes: paid {} place(s)", paid);
    Ok(())
}
