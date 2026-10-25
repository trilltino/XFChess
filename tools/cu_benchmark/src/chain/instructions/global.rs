use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

use super::common::*;

fn global_session_pda(program_id: &Pubkey, player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"global_session", player.as_ref()], program_id).0
}

#[allow(clippy::too_many_arguments)]
pub fn authorize_global_session_ix(
    program_id: Pubkey,
    player: Pubkey,
    session_key: Pubkey,
    duration_secs: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
    games: Option<u16>,
    deposit_lamports: u64,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);

    let mut data = anchor_discriminator("authorize_global_session").to_vec();
    // AuthorizeGlobalSessionArgs (Borsh)
    data.extend_from_slice(session_key.as_ref());
    match duration_secs {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match spending_limit {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match max_wager {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    match games {
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
        None => data.push(0),
    }
    data.extend_from_slice(&deposit_lamports.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn global_create_game_ix(
    program_id: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("global_create_game_v2").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(session_signer, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
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
    })
}

pub fn global_join_game_ix(
    program_id: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let session_pda = global_session_pda(&program_id, &player);
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("global_join_game_v2").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(session_signer, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new(escrow_pda, false),
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
    })
}
