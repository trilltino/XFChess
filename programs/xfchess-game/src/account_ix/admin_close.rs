//! Devnet reset tool: the treasury authority closes any program-owned account, so a test
//! deployment can be wiped without a new program ID. Typed accounts cannot be closed this way
//! after a layout change, so the account is handled as raw bytes.
//!
//! Safeguards (see docs/DEVNET_RESET.md):
//! - compiled only with the opt-in `devnet-reset` feature, never together with `mainnet`;
//! - the lamports can only go to the signing authority, never to a caller-chosen address;
//! - `ProgramConfig` and delegated accounts cannot be closed;
//! - the intended use is a short window: deploy this build, wipe, then redeploy a normal build.

use crate::errors::GameErrorCode;
use crate::state::ProgramConfig;
use anchor_lang::prelude::*;

#[event]
pub struct AdminAccountClosed {
    pub account: Pubkey,
    pub recipient: Pubkey,
    pub lamports: u64,
}

#[derive(Accounts)]
pub struct AdminCloseAccount<'info> {
    /// Receives the closed account's lamports.
    #[account(
        mut,
        address = crate::constants::treasury_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    /// CHECK: Any account owned by this program except `ProgramConfig`; checked in the handler.
    #[account(mut)]
    pub target: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<AdminCloseAccount>) -> Result<()> {
    let target = ctx.accounts.target.to_account_info();
    let recipient = ctx.accounts.authority.to_account_info();

    // Delegated accounts are owned by the delegation program, so this also refuses them.
    require_keys_eq!(*target.owner, crate::ID, GameErrorCode::InvalidAccountOwner);
    {
        let data = target.try_borrow_data()?;
        require!(data.len() >= 8, GameErrorCode::InvalidArgument);
        // The config holds the authority keys; closing it would brick the program.
        require!(
            &data[..8] != ProgramConfig::DISCRIMINATOR,
            GameErrorCode::UnauthorizedAccess
        );
    }

    let lamports = target.lamports();
    **recipient.try_borrow_mut_lamports()? = recipient
        .lamports()
        .checked_add(lamports)
        .ok_or(GameErrorCode::InsufficientFunds)?;
    **target.try_borrow_mut_lamports()? = 0;
    target.assign(&anchor_lang::system_program::ID);
    target.resize(0)?;

    emit!(AdminAccountClosed {
        account: target.key(),
        recipient: recipient.key(),
        lamports,
    });
    Ok(())
}
