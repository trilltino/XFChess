use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    #[account(mut)]
    pub base_account: UncheckedAccount<'info>,
    #[account()]
    pub buffer: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: UncheckedAccount<'info>,
    pub system_program: UncheckedAccount<'info>,
}
