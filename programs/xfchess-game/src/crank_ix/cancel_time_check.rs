use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CancelTimeCheckArgs {
    pub task_id: u64,
}

pub fn cancel_time_check_crank(
    ctx: Context<CancelTimeCheck>,
    args: CancelTimeCheckArgs,
) -> Result<()> {
    let cancel_ix = crate::magicblock::crank::build_time_check_cancel_instruction(
        ctx.accounts.payer.key(),
        ctx.accounts.game.key(),
        args.task_id,
    )?;

    invoke_signed(
        &cancel_ix,
        &[
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.game.to_account_info(),
        ],
        &[],
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct CancelTimeCheck<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, crate::state::Game>,

    #[account(address = MAGIC_PROGRAM_ID)]
    pub magic_program: AccountInfo<'info>,
}

// Crank-specific small error enums removed; use the central `GameErrorCode`
// in `crate::errors` (e.g. `GameErrorCode::InvalidArgument`) to avoid
// multiple #[error_code] enums in the crate which the IDL builder rejects.
