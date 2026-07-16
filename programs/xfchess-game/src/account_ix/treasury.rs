//! Withdraw accumulated platform fees from the system-owned treasury vault.
//!
//! The treasury vault (seeds `[TREASURY_VAULT_SEED]`) accrues PvP platform fees,
//! dispute-resolution fees, and forfeited dispute bonds from game settlement.
//! It is System-owned, so — exactly like the per-game wager escrow — lamports may
//! only leave through a `system_program::transfer` CPI signed with the vault seeds.
//! A direct lamport decrement would fail the runtime's ownership check.

use crate::constants::*;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

#[event]
pub struct TreasuryWithdrawn {
    pub authority: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub remaining: u64,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct WithdrawTreasury<'info> {
    /// System-owned platform treasury vault — the destination of all PvP
    /// platform/dispute fees. Seeded PDA, so it cannot be substituted.
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    /// Only the dedicated treasury authority may withdraw. Kept separate from
    /// `vps_authority` so treasury access uses its own dedicated wallet without
    /// touching the result-signing key.
    #[account(
        mut,
        address = crate::constants::treasury_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    /// Destination wallet for the withdrawn fees.
    #[account(mut)]
    pub destination: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    require!(amount > 0, GameErrorCode::InvalidArgument);

    let vault = &ctx.accounts.treasury_vault;
    // Keep the vault rent-exempt so partial withdrawals don't garbage-collect it
    // while fees are still accumulating between claims.
    let rent_min = Rent::get()?.minimum_balance(vault.data_len());
    let remaining = vault
        .lamports()
        .checked_sub(amount)
        .ok_or(GameErrorCode::InsufficientFunds)?;
    require!(remaining >= rent_min, GameErrorCode::InsufficientFunds);

    // treasury_vault is System-owned, so lamports must leave via a signed CPI
    // transfer — same mechanism as pay_from_game_escrow for the wager escrow.
    let bump = ctx.bumps.treasury_vault;
    let signer: &[&[&[u8]]] = &[&[TREASURY_VAULT_SEED, &[bump]]];
    system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_vault.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    emit!(TreasuryWithdrawn {
        authority: ctx.accounts.authority.key(),
        destination: ctx.accounts.destination.key(),
        amount,
        remaining,
    });
    Ok(())
}
