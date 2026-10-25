use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    /// CHECK: handler verifies the Game PDA; the SDK restores ownership and data.
    #[account(mut)]
    pub base_account: UncheckedAccount<'info>,
    /// CHECK: canonical buffer; SDK also requires delegation ownership and signature.
    #[account(
        seeds = [b"undelegate-buffer", base_account.key().as_ref()],
        bump,
        seeds::program = ephemeral_rollups_sdk::id(),
    )]
    pub buffer: UncheckedAccount<'info>,
    /// CHECK: SDK System CPI enforces authorization when a rent transfer is needed.
    #[account(mut)]
    pub payer: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
