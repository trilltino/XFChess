use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    /// CHECK: delegated account being restored
    #[account(mut)]
    pub base_account: AccountInfo<'info>,
    /// CHECK: buffer account
    #[account()]
    pub buffer: AccountInfo<'info>,
    /// CHECK: payer
    #[account(mut)]
    pub payer: AccountInfo<'info>,
    /// CHECK: system program
    pub system_program: AccountInfo<'info>,
}
