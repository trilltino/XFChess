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
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    #[account(
        mut,
        address = crate::constants::treasury_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
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
            System::id(),
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
