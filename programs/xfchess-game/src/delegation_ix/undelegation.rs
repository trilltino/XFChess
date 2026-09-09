use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    #[account(mut)]
    pub base_account: AccountInfo<'info>,
    #[account()]
    pub buffer: AccountInfo<'info>,
    #[account(mut)]
    pub payer: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
}
