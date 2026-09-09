use crate::constants::{ER_COMMIT_FREQUENCY_MS, GAME_SEED};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::cpi::{delegate_account, DelegateAccounts, DelegateConfig};
use ephemeral_rollups_sdk::ephem::{FoldableIntentBuilder, MagicIntentBundleBuilder};
use ephemeral_rollups_sdk::instruction_builder;

pub fn game_seed_bytes(game_id: u64) -> [u8; 8] {
    game_id.to_le_bytes()
}

pub fn default_delegate_config() -> DelegateConfig {
    DelegateConfig {
        commit_frequency_ms: ER_COMMIT_FREQUENCY_MS,
        validator: None,
    }
}

pub fn delegate_game_pda<'a, 'info>(
    accounts: DelegateAccounts<'a, 'info>,
    game_id_bytes: &[u8; 8],
) -> Result<()> {
    let seeds: &[&[u8]] = &[b"game", game_id_bytes];
    delegate_account(accounts, seeds, default_delegate_config())?;
    Ok(())
}

pub fn commit_and_undelegate_game_pda<'info>(
    payer: &AccountInfo<'info>,
    game: &AccountInfo<'info>,
    magic_context: &AccountInfo<'info>,
    magic_program: &AccountInfo<'info>,
) -> Result<()> {
    MagicIntentBundleBuilder::new(payer.clone(), magic_context.clone(), magic_program.clone())
        .commit_and_undelegate(&[game.clone()])
        .build_and_invoke()?;
    Ok(())
}

fn require_accounts_match_instruction(ix: &Instruction, infos: &[AccountInfo]) -> Result<()> {
    require_eq!(
        ix.accounts.len(),
        infos.len(),
        crate::errors::GameErrorCode::InvalidArgument
    );
    for (meta, info) in ix.accounts.iter().zip(infos.iter()) {
        require_keys_eq!(
            meta.pubkey,
            info.key(),
            crate::errors::GameErrorCode::InvalidArgument
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn request_force_undelegate<'info>(
    payer: &AccountInfo<'info>,
    game: &AccountInfo<'info>,
    owner_program: &AccountInfo<'info>,
    undelegation_request: &AccountInfo<'info>,
    delegation_record: &AccountInfo<'info>,
    delegation_metadata: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    game_id_bytes: &[u8; 8],
    game_bump: u8,
) -> Result<()> {
    let ix =
        instruction_builder::request_undelegation(payer.key(), game.key(), owner_program.key());
    let account_infos = [
        payer.clone(),
        game.clone(),
        owner_program.clone(),
        undelegation_request.clone(),
        delegation_record.clone(),
        delegation_metadata.clone(),
        system_program.clone(),
    ];
    require_accounts_match_instruction(&ix, &account_infos)?;
    let seeds: &[&[u8]] = &[GAME_SEED, game_id_bytes, &[game_bump]];
    invoke_signed(&ix, &account_infos, &[seeds])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn force_undelegate_after_timeout<'info>(
    game: &AccountInfo<'info>,
    owner_program: &AccountInfo<'info>,
    undelegation_request: &AccountInfo<'info>,
    delegation_record: &AccountInfo<'info>,
    delegation_metadata: &AccountInfo<'info>,
    delegation_rent_payer: &AccountInfo<'info>,
    commit_state: &AccountInfo<'info>,
    commit_record: &AccountInfo<'info>,
    commit_reimbursement: &AccountInfo<'info>,
    game_id_bytes: &[u8; 8],
    game_bump: u8,
) -> Result<()> {
    let ix = instruction_builder::undelegate_with_rollback_after_timeout(
        game.key(),
        owner_program.key(),
        delegation_rent_payer.key(),
        commit_reimbursement.key(),
    );
    let account_infos = [
        game.clone(),
        owner_program.clone(),
        undelegation_request.clone(),
        delegation_record.clone(),
        delegation_metadata.clone(),
        delegation_rent_payer.clone(),
        commit_state.clone(),
        commit_record.clone(),
        commit_reimbursement.clone(),
    ];
    require_accounts_match_instruction(&ix, &account_infos)?;
    let seeds: &[&[u8]] = &[GAME_SEED, game_id_bytes, &[game_bump]];
    invoke_signed(&ix, &account_infos, &[seeds])?;
    Ok(())
}
