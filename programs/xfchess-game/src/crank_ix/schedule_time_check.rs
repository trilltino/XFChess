//! Schedule a crank to automatically check game time controls.
//! 
//! This instruction schedules a recurring task that will automatically
//! flag players who exceed their time limit.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID;
use magicblock_magic_program_api::{args::ScheduleTaskArgs, instruction::MagicBlockInstruction};

/// Arguments for scheduling a time check crank
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ScheduleTimeCheckArgs {
    /// Unique task identifier (typically game_id)
    pub task_id: u64,
    /// Milliseconds between time checks (e.g., 1000 for 1 second)
    pub check_interval_millis: u64,
    /// Number of times to run the check (0 = unlimited until cancelled)
    pub iterations: u64,
}

/// Schedule an automatic time check crank for a game.
/// 
/// This schedules a recurring task on the Ephemeral Rollup that will
/// automatically check if a player has exceeded their time limit.
/// 
/// Must be sent to the Ephemeral Rollup (not base layer).
pub fn schedule_time_check_crank(
    ctx: Context<ScheduleTimeCheck>,
    args: ScheduleTimeCheckArgs,
) -> Result<()> {
    // Build the crank instruction that will be called automatically
    let crank_ix = Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(ctx.accounts.game.key(), false),
            AccountMeta::new_readonly(ctx.accounts.white.key(), false),
            AccountMeta::new_readonly(ctx.accounts.black.key(), false),
        ],
        data: anchor_lang::InstructionData::data(&crate::instruction::CrankTimeCheck {}),
    };

    // Serialize the schedule task instruction data
    let ix_data = bincode::serialize(&MagicBlockInstruction::ScheduleTask(ScheduleTaskArgs {
        task_id: args.task_id,
        execution_interval_millis: args.check_interval_millis,
        iterations: args.iterations,
        instructions: vec![crank_ix],
    })).map_err(|_| ErrorCode::InvalidArgument)?;

    // Build the schedule instruction for the MagicBlock program
    let schedule_ix = Instruction::new_with_bytes(
        MAGIC_PROGRAM_ID,
        &ix_data,
        vec![
            AccountMeta::new(ctx.accounts.payer.key(), true),
            AccountMeta::new(ctx.accounts.game.key(), false),
        ],
    );

    // Invoke the schedule instruction
    invoke_signed(
        &schedule_ix,
        &[
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.game.to_account_info(),
        ],
        &[],
    )?;

    msg!("Scheduled time check crank for game {}: interval={}ms, iterations={}", 
        args.task_id, args.check_interval_millis, args.iterations);

    Ok(())
}

#[derive(Accounts)]
pub struct ScheduleTimeCheck<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// The game account to monitor for time controls
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, crate::state::Game>,
    
    /// CHECK: White player (for reference)
    pub white: AccountInfo<'info>,
    
    /// CHECK: Black player (for reference)
    pub black: AccountInfo<'info>,
    
    /// CHECK: MagicBlock program for scheduling
    #[account(address = MAGIC_PROGRAM_ID)]
    pub magic_program: AccountInfo<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid argument")]
    InvalidArgument,
}
