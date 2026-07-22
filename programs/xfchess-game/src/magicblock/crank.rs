//! MagicBlock scheduled-task adapter.

#[cfg(feature = "cranks")]
use anchor_lang::prelude::*;
#[cfg(feature = "cranks")]
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
#[cfg(feature = "cranks")]
use magicblock_magic_program_api::{args::ScheduleTaskArgs, instruction::MagicBlockInstruction};

#[cfg(feature = "cranks")]
pub fn build_time_check_schedule_instruction(
    payer: Pubkey,
    game: Pubkey,
    white: Pubkey,
    black: Pubkey,
    task_id: u64,
    check_interval_millis: u64,
    iterations: u64,
) -> Result<Instruction> {
    let crank_ix = Instruction {
        program_id: crate::ID,
        accounts: vec![
            AccountMeta::new(game, false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
        ],
        data: anchor_lang::prelude::borsh::to_vec(&())
            .map_err(|_| crate::crank_ix::schedule_time_check::ErrorCode::InvalidArgument)?,
    };

    let ix_data = bincode::serialize(&MagicBlockInstruction::ScheduleTask(ScheduleTaskArgs {
        task_id: task_id as i64,
        execution_interval_millis: check_interval_millis as i64,
        iterations: iterations as i64,
        instructions: vec![crank_ix],
    }))
    .map_err(|_| crate::crank_ix::schedule_time_check::ErrorCode::InvalidArgument)?;

    Ok(Instruction::new_with_bytes(
        ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID,
        &ix_data,
        vec![AccountMeta::new(payer, true), AccountMeta::new(game, false)],
    ))
}
