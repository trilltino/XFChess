use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

use super::common::*;

pub fn authorize_session_key_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
    session_pubkey: Pubkey,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;

    // authorize_session_key takes game_id and session_pubkey; lifetime is fixed on-chain.
    let mut data = anchor_discriminator("authorize_session_key").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(session_pubkey.as_ref());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn delegate_game_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
    valid_until: i64,
) -> Result<Instruction> {
    let buffer_pda = {
        let pda = ephemeral_rollups_sdk::pda::delegate_buffer_pda_from_delegated_account_and_owner_program(
            &game_pda.to_bytes().into(),
            &program_id.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_record = {
        let pda = ephemeral_rollups_sdk::pda::delegation_record_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_metadata = {
        let pda = ephemeral_rollups_sdk::pda::delegation_metadata_pda_from_delegated_account(
            &game_pda.to_bytes().into(),
        );
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_program: Pubkey = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh".parse()?;

    let mut data = anchor_discriminator("delegate_game_v2").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&valid_until.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new(
                Pubkey::find_program_address(
                    &[b"recovery_identity", &game_id.to_le_bytes()],
                    &program_id,
                )
                .0,
                false,
            ),
        ],
        data,
    })
}

pub fn undelegate_game_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let magic_context: Pubkey = "MagicContext1111111111111111111111111111111".parse()?;
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

pub fn schedule_time_check_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    payer: Pubkey,
    white: Pubkey,
    black: Pubkey,
    game_id: u64,
    interval_ms: u64,
) -> Result<Instruction> {
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    // ScheduleTimeCheckArgs { task_id: u64, check_interval_millis: u64, iterations: u64 }
    let mut data = anchor_discriminator("schedule_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&interval_ms.to_le_bytes());
    let iterations = 0u64; // unlimited
    data.extend_from_slice(&iterations.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

pub fn crank_time_check_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    white: Pubkey,
    black: Pubkey,
) -> Result<Instruction> {
    let data = anchor_discriminator("crank_time_check").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(white, false),
            AccountMeta::new_readonly(black, false),
        ],
        data,
    })
}

pub fn cancel_time_check_ix(
    program_id: Pubkey,
    payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let magic_program: Pubkey = "Magic11111111111111111111111111111111111111".parse()?;

    let mut data = anchor_discriminator("cancel_time_check").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes()); // task_id

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

pub fn request_force_undelegate_ix(
    program_id: Pubkey,
    game_id: u64,
    payer: Pubkey,
) -> Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID.parse()?;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let undelegation_request = Pubkey::new_from_array(
        undelegation_request_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_record = Pubkey::new_from_array(
        delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_metadata = Pubkey::new_from_array(
        delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );

    let mut data = anchor_discriminator("request_force_undelegate").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new_readonly(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(delegation_program_id, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn force_undelegate_after_timeout_ix(
    program_id: Pubkey,
    game_id: u64,
    payer: Pubkey,
) -> Result<Instruction> {
    use ephemeral_rollups_sdk::pda::{
        commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    };

    let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID.parse()?;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let undelegation_request = Pubkey::new_from_array(
        undelegation_request_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_record = Pubkey::new_from_array(
        delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let delegation_metadata = Pubkey::new_from_array(
        delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let commit_state = Pubkey::new_from_array(
        commit_state_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );
    let commit_record = Pubkey::new_from_array(
        commit_record_pda_from_delegated_account(&game_pda.to_bytes().into()).to_bytes(),
    );

    let mut data = anchor_discriminator("force_undelegate_after_timeout").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(undelegation_request, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(payer, false),
            AccountMeta::new(commit_state, false),
            AccountMeta::new(commit_record, false),
            AccountMeta::new(payer, false), // commit_reimbursement placeholder
            AccountMeta::new_readonly(delegation_program_id, false),
        ],
        data,
    })
}

pub fn recover_stuck_delegation_ix(
    program_id: Pubkey,
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    dispute_authority: Pubkey,
) -> Instruction {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("recover_stuck_delegation_v2").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(white, false),
            AccountMeta::new(black, false),
            AccountMeta::new_readonly(dispute_authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(
                Pubkey::find_program_address(
                    &[b"recovery_identity", &game_id.to_le_bytes()],
                    &program_id,
                )
                .0,
                false,
            ),
        ],
        data,
    }
}
