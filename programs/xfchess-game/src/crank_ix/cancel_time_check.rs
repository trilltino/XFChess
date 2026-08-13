//! Cancel a previously scheduled time-check crank.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID;

/// Arguments for cancelling a scheduled time check crank.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CancelTimeCheckArgs {
    /// Task identifier to cancel (the same `task_id` — typically game_id —
    /// used in the matching `schedule_time_check` call).
    pub task_id: u64,
}

/// Cancel a previously scheduled time-check crank for a game.
///
/// Must be sent to the Ephemeral Rollup (not base layer) and signed by the
/// same payer that originally scheduled the task — MagicBlock's `CancelTask`
/// requires the task authority to match the original scheduler. Called when
/// a game ends (resign, timeout, or undelegation) so the recurring task
/// doesn't keep firing against a game that's no longer active/delegated.
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

    /// The game account whose time-check task is being cancelled.
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, crate::state::Game>,

    /// CHECK: MagicBlock program for task cancellation
    #[account(address = MAGIC_PROGRAM_ID)]
    pub magic_program: AccountInfo<'info>,
}

// Crank-specific small error enums removed; use the central `GameErrorCode`
// in `crate::errors` (e.g. `GameErrorCode::InvalidArgument`) to avoid
// multiple #[error_code] enums in the crate which the IDL builder rejects.
