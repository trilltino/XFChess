use super::super::common::*;
use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

#[allow(clippy::too_many_arguments)]
pub fn authorize_tournament_session_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    player: Pubkey,
    session_key: Pubkey,
    spending_limit: u64,
    max_wager: u64,
    duration_secs: i64,
    deposit_lamports: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("authorize_tournament_session").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    // AuthorizeTournamentSessionArgs Borsh serialization
    data.extend_from_slice(session_key.as_ref());
    data.push(1); // Option::Some for duration_secs
    data.extend_from_slice(&duration_secs.to_le_bytes());
    data.push(1); // Option::Some for spending_limit
    data.extend_from_slice(&spending_limit.to_le_bytes());
    data.push(1); // Option::Some for max_wager
    data.extend_from_slice(&max_wager.to_le_bytes());
    data.extend_from_slice(&deposit_lamports.to_le_bytes()); // deposit_lamports to pre-fund session rent

    Ok(Instruction {
        program_id,
        accounts: vec![
            // tournament + all 4 shards are read-only on-chain
            // (authorize_tournament_session.rs) — only session_delegation is written.
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn session_create_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    white_session: Pubkey,
    white_player: Pubkey,
    wager: u64,
    platform_fee: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            white_player.as_ref(),
        ],
        &program_id,
    )
    .0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    // Encode session_create_game to match the program; platform_fee is u64.
    let mut data = anchor_discriminator("session_create_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager.to_le_bytes());
    // MatchType::Free = 0
    data.push(0);
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.extend_from_slice(&60u64.to_le_bytes()); // base_time_seconds — 1 min, synthetic instant-move benchmark
    data.extend_from_slice(&0u16.to_le_bytes()); // increment_seconds

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new_readonly(white_session, true),
            AccountMeta::new_readonly(white_player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn session_join_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    max_players: u16,
    game_id: u64,
    session_key: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            player.as_ref(),
        ],
        &program_id,
    )
    .0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("session_join_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            // tournament + all 4 shards are read-only on-chain (session_join_game.rs).
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new_readonly(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new_readonly(session_key, true),
            AccountMeta::new_readonly(player, false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(white_profile_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}
