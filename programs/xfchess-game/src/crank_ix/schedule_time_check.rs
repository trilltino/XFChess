use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ScheduleTimeCheckArgs {
    pub task_id: u64,
    pub check_interval_millis: u64,
    pub iterations: u64,
}

pub fn schedule_time_check_crank(
    ctx: Context<ScheduleTimeCheck>,
    args: ScheduleTimeCheckArgs,
) -> Result<()> {
    let schedule_ix = crate::magicblock::crank::build_time_check_schedule_instruction(
        ctx.accounts.payer.key(),
        ctx.accounts.game.key(),
        ctx.accounts.white.key(),
        ctx.accounts.black.key(),
        args.task_id,
        args.check_interval_millis,
        args.iterations,
    )?;

    // Invoke the schedule instruction
    invoke_signed(
        &schedule_ix,
        &[
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.game.to_account_info(),
        ],
        &[],
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ScheduleTimeCheck<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, crate::state::Game>,

    #[account(constraint = white.key() == game.white @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub white: AccountInfo<'info>,

    #[account(constraint = black.key() == game.black @ crate::errors::GameErrorCode::InvalidPlayerAccount)]
    pub black: AccountInfo<'info>,

    #[account(address = MAGIC_PROGRAM_ID)]
    pub magic_program: AccountInfo<'info>,
}

// Crank-specific small error enums removed; use the central `GameErrorCode`
// in `crate::errors` (e.g. `GameErrorCode::InvalidArgument`) to avoid
// multiple #[error_code] enums in the crate which the IDL builder rejects.
