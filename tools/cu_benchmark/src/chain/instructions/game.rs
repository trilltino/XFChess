use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
#[allow(deprecated)]
use solana_system_interface::program as system_program;

use super::common::*;

pub fn init_profile_ix(
    program_id: Pubkey,
    player: Pubkey,
    username: String,
    country: String,
    date_of_birth: i64,
) -> Result<Instruction> {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let username_record_pda =
        Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id).0;

    let mut data = anchor_discriminator("init_profile").to_vec();
    data.extend(borsh_string(&username));
    data.extend(borsh_string(&country));
    data.extend(date_of_birth.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(username_record_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Encode create_game arguments to match game_ix/create.rs; platform_fee is u64.
    let mut data = anchor_discriminator("create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn join_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("join_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)] // Mirrors the instruction's accounts and payload.
pub fn record_move_ix(
    program_id: Pubkey,
    session_key: Pubkey,
    wallet_player: Pubkey,
    game_id: u64,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    signature: Option<Vec<u8>>,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_pda = Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            wallet_player.as_ref(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&move_uci);
    data.extend_from_slice(&next_board);
    data.extend_from_slice(&nonce.to_le_bytes());

    if let Some(sig) = signature {
        data.push(1);
        data.extend_from_slice(&(sig.len() as u32).to_le_bytes());
        data.extend_from_slice(&sig);
    } else {
        data.push(0);
    }
    data.push(0);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(session_key, true),
            AccountMeta::new_readonly(session_pda, false),
        ],
        data,
    })
}

pub fn finalize_game_ix(
    program_id: Pubkey,
    game_id: u64,
    white_pubkey: Pubkey,
    black_pubkey: Pubkey,
    fee_payer: Pubkey,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile =
        Pubkey::find_program_address(&[PROFILE_SEED, white_pubkey.as_ref()], &program_id).0;
    let black_profile =
        Pubkey::find_program_address(&[PROFILE_SEED, black_pubkey.as_ref()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(white_pubkey, false),
            AccountMeta::new(black_pubkey, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            // fee_payer is a SystemAccount, not a required signer.
            AccountMeta::new(fee_payer, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn resign_game_ix(program_id: Pubkey, game_id: u64, player: Pubkey) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("resign").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    // ResignGame accounts are writable game and read-only player signer.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player, true),
        ],
        data,
    })
}

// Global sessions fund game rent and wagers from their delegation vault.
// Match the current program argument layout, including platform_fee: u64.
