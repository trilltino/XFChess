use crate::constants::GAME_SEED;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RequestForceUndelegateCtx<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
    pub owner_program: AccountInfo<'info>,
    #[account(mut)]
    pub undelegation_request_pda: AccountInfo<'info>,
    pub delegation_record_pda: AccountInfo<'info>,
    #[account(mut)]
    pub delegation_metadata_pda: AccountInfo<'info>,
    #[account(address = ephemeral_rollups_sdk::id())]
    pub delegation_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_request_force_undelegate(
    ctx: Context<RequestForceUndelegateCtx>,
    game_id: u64,
) -> Result<()> {
    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = ctx.bumps.game;
    crate::magicblock::delegation::request_force_undelegate(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.owner_program.to_account_info(),
        &ctx.accounts.undelegation_request_pda.to_account_info(),
        &ctx.accounts.delegation_record_pda.to_account_info(),
        &ctx.accounts.delegation_metadata_pda.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &game_id_bytes,
        game_bump,
    )
}

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ForceUndelegateAfterTimeoutCtx<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: AccountInfo<'info>,
    #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
    pub owner_program: AccountInfo<'info>,
    #[account(mut)]
    pub undelegation_request_pda: AccountInfo<'info>,
    #[account(mut)]
    pub delegation_record_pda: AccountInfo<'info>,
    #[account(mut)]
    pub delegation_metadata_pda: AccountInfo<'info>,
    #[account(mut)]
    pub delegation_rent_payer: AccountInfo<'info>,
    #[account(mut)]
    pub commit_state_pda: AccountInfo<'info>,
    #[account(mut)]
    pub commit_record_pda: AccountInfo<'info>,
    #[account(mut)]
    pub commit_reimbursement: AccountInfo<'info>,
    #[account(address = ephemeral_rollups_sdk::id())]
    pub delegation_program: AccountInfo<'info>,
}

pub fn handler_force_undelegate_after_timeout(
    ctx: Context<ForceUndelegateAfterTimeoutCtx>,
    game_id: u64,
) -> Result<()> {
    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = ctx.bumps.game;
    crate::magicblock::delegation::force_undelegate_after_timeout(
        &ctx.accounts.game.to_account_info(),
        &ctx.accounts.owner_program.to_account_info(),
        &ctx.accounts.undelegation_request_pda.to_account_info(),
        &ctx.accounts.delegation_record_pda.to_account_info(),
        &ctx.accounts.delegation_metadata_pda.to_account_info(),
        &ctx.accounts.delegation_rent_payer.to_account_info(),
        &ctx.accounts.commit_state_pda.to_account_info(),
        &ctx.accounts.commit_record_pda.to_account_info(),
        &ctx.accounts.commit_reimbursement.to_account_info(),
        &game_id_bytes,
        game_bump,
    )
}
