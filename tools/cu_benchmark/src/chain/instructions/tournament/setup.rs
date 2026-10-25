use super::super::common::*;
use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_system_interface::program as system_program;

#[allow(clippy::too_many_arguments)] // Mirrors the instruction's accounts and payload.
pub fn initialize_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    max_players: u16,
    rounds: Option<u8>,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    platform_fee: u64,
    winner_takes_all: bool,
    host_treasury: Pubkey,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let usdc_prize_escrow_authority = Pubkey::find_program_address(
        &[b"t_usdc_prize", &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let usdc_mint: Pubkey = WRAPPED_SOL_MINT.parse()?;
    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".parse()?;
    let associated_token_program: Pubkey =
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse()?;
    let usdc_prize_escrow = Pubkey::find_program_address(
        &[
            &usdc_prize_escrow_authority.to_bytes(),
            &token_program.to_bytes(),
            &usdc_mint.to_bytes(),
        ],
        &associated_token_program,
    )
    .0;

    // Build instruction data matching handler signature
    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());
    match rounds {
        Some(rounds) => {
            data.push(TOURNAMENT_TYPE_SWISS);
            data.push(rounds);
        }
        None => {
            data.push(TOURNAMENT_TYPE_SINGLE_ELIMINATION);
        }
    }
    data.extend_from_slice(&elo_min.to_le_bytes());
    data.extend_from_slice(&elo_max.to_le_bytes());
    data.extend_from_slice(&min_players.to_le_bytes());
    for share in &prize_shares {
        data.extend_from_slice(&share.to_le_bytes());
    }
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.push(if winner_takes_all { 1 } else { 0 });
    data.extend_from_slice(&host_treasury.to_bytes());
    // Option<Pubkey> — Some(wrapped_sol_mint)
    data.push(1);
    data.extend_from_slice(&usdc_mint.to_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(usdc_prize_escrow_authority, false),
            AccountMeta::new(usdc_prize_escrow, false),
            AccountMeta::new_readonly(usdc_mint, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// Select the shard-init instruction by capacity: <=64 uses one shard,
// <=128 two, and 256 four. Each instruction has its own capacity constraint.

pub fn initialize_shards_small_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_shards_small").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn initialize_shards_medium_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_shards_medium").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 1), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn initialize_tournament_shards_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_tournament_shards").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            // `tournament` is read-only here (initialize_shards.rs:133-139) — the
            // shards are what get initialized, not the tournament record itself.
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 1), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 2), false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 3), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn initialize_shards_for_size_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    match required_shards(max_players) {
        1 => initialize_shards_small_ix(program_id, authority, tournament_id),
        2 => initialize_shards_medium_ix(program_id, authority, tournament_id),
        _ => initialize_tournament_shards_ix(program_id, authority, tournament_id),
    }
}

pub fn initialize_tournament_escrow_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_tournament_escrow").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn register_player_ix(
    program_id: Pubkey,
    player: Pubkey,
    host_treasury: Pubkey,
    tournament_id: u64,
    max_players: u16,
    elo: u32,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let mut data = anchor_discriminator("register_player").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    // Keep the ten RegisterPlayer accounts in program order. host_treasury
    // is unchecked but constrained, not a signer.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(host_treasury, false),
            AccountMeta::new_readonly(system_program::id(), false),
            // Pass accounts. The binding PDA is always required (empty and system-owned for a
            // tournament without a pass); the three optional ones are passed as `None`.
            AccountMeta::new_readonly(
                Pubkey::find_program_address(
                    &[b"t_pass", &tournament_id.to_le_bytes()],
                    &program_id,
                )
                .0,
                false,
            ),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new_readonly(program_id, false),
        ],
        data,
    })
}

pub fn start_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    host_treasury: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    // StartTournament sweeps escrow into the host treasury; include both account metas.
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_pda(&program_id, tournament_id, 0), false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(host_treasury, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}
