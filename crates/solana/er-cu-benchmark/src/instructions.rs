//! Raw Anchor instruction builders for XFChess benchmark tests.

use anyhow::Result;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
#[allow(deprecated)]
use solana_sdk::system_program;

const GAME_SEED: &[u8] = b"game";
const MOVE_LOG_SEED: &[u8] = b"move_log";
const PROFILE_SEED: &[u8] = b"profile";
const USERNAME_SEED: &[u8] = b"username";
const WAGER_ESCROW_SEED: &[u8] = b"escrow";
const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
const TOURNAMENT_SEED: &[u8] = b"tournament";
const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

fn anchor_discriminator(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

// ---------------------------------------------------------------------------
// init_profile
// ---------------------------------------------------------------------------
pub fn init_profile_ix(
    program_id: Pubkey,
    player: Pubkey,
    username: String,
    country: String,
    date_of_birth: i64,
) -> Result<Instruction> {
    let player_profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let username_record_pda = Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id).0;

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

// ---------------------------------------------------------------------------
// create_game
// ---------------------------------------------------------------------------
pub fn create_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    country: &str,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    data.extend_from_slice(&(country.len() as u32).to_le_bytes());
    data.extend_from_slice(country.as_bytes());
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

// ---------------------------------------------------------------------------
// join_game
// ---------------------------------------------------------------------------
pub fn join_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let player_profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, white_player.as_ref()], &program_id).0;

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

// ---------------------------------------------------------------------------
// record_move
// ---------------------------------------------------------------------------
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
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_pda = Pubkey::find_program_address(
        &[b"session_delegation", &game_id.to_le_bytes(), wallet_player.as_ref()],
        &program_id,
    ).0;

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

// ---------------------------------------------------------------------------
// finalize_game
// ---------------------------------------------------------------------------
pub fn finalize_game_ix(
    program_id: Pubkey,
    game_id: u64,
    white_pubkey: Pubkey,
    black_pubkey: Pubkey,
    fee_payer: Pubkey,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let white_profile = Pubkey::find_program_address(&[PROFILE_SEED, white_pubkey.as_ref()], &program_id).0;
    let black_profile = Pubkey::find_program_address(&[PROFILE_SEED, black_pubkey.as_ref()], &program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;
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
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_session_key
// ---------------------------------------------------------------------------
pub fn authorize_session_key_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
    session_pubkey: Pubkey,
    duration_seconds: i64,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[SESSION_DELEGATION_SEED, &game_id.to_le_bytes(), player.as_ref()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("authorize_session_key").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(session_pubkey.as_ref());
    data.extend_from_slice(&duration_seconds.to_le_bytes());

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

// ---------------------------------------------------------------------------
// delegate_game (ER-specific)
// ---------------------------------------------------------------------------
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
        let pda = ephemeral_rollups_sdk::pda::delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into());
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_metadata = {
        let pda = ephemeral_rollups_sdk::pda::delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into());
        Pubkey::new_from_array(pda.to_bytes())
    };
    let delegation_program: Pubkey = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh".parse()?;

    let mut data = anchor_discriminator("delegate_game").to_vec();
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
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// undelegate_game (ER-specific)
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// schedule_time_check (ER crank)
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// crank_time_check (ER crank)
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// commit_move_batch (ER)
// ---------------------------------------------------------------------------
pub fn commit_move_batch_ix(
    program_id: Pubkey,
    game_pda: Pubkey,
    fee_payer: Pubkey,
) -> Result<Instruction> {
    let data = anchor_discriminator("commit_move_batch").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// initialize_tournament
// ---------------------------------------------------------------------------
/// TournamentType enum discriminant for Swiss variant.
const TOURNAMENT_TYPE_SWISS: u8 = 0;

/// Wrapped SOL mint — used as a placeholder "USDC" mint for benchmarks.
const WRAPPED_SOL_MINT: &str = "So11111111111111111111111111111111111111112";

pub fn initialize_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    max_players: u16,
    rounds: u8,
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
    ).0;
    let usdc_prize_escrow_authority = Pubkey::find_program_address(
        &[b"t_usdc_prize", &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let usdc_mint: Pubkey = WRAPPED_SOL_MINT.parse()?;
    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".parse()?;
    let associated_token_program: Pubkey = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse()?;
    let usdc_prize_escrow = Pubkey::find_program_address(
        &[
            &usdc_prize_escrow_authority.to_bytes(),
            &token_program.to_bytes(),
            &usdc_mint.to_bytes(),
        ],
        &associated_token_program,
    ).0;

    // Build instruction data matching handler signature
    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());
    // TournamentType::Swiss { rounds }
    data.push(TOURNAMENT_TYPE_SWISS);
    data.push(rounds);
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

// ---------------------------------------------------------------------------
// initialize_tournament_shards
// ---------------------------------------------------------------------------
pub fn initialize_tournament_shards_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("initialize_tournament_shards").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// initialize_tournament_escrow
// ---------------------------------------------------------------------------
pub fn initialize_tournament_escrow_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;

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

// ---------------------------------------------------------------------------
// register_player
// ---------------------------------------------------------------------------
pub fn register_player_ix(
    program_id: Pubkey,
    player: Pubkey,
    treasury_vault: Pubkey,
    tournament_id: u64,
    elo: u32,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    ).0;
    // Program constraint requires treasury_vault == platform_treasury_vault
    let platform_treasury_vault = treasury_vault;

    let mut data = anchor_discriminator("register_player").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
            AccountMeta::new(treasury_vault, true),
            AccountMeta::new_readonly(platform_treasury_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// start_tournament
// ---------------------------------------------------------------------------
pub fn start_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// record_match_result
// ---------------------------------------------------------------------------
pub fn record_match_result_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    match_index: u8,
    winner: Pubkey,
    game_pda: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("record_match_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.push(match_index);
    data.extend_from_slice(winner.as_ref());
    data.extend_from_slice(game_pda.as_ref());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_tournament_session
// ---------------------------------------------------------------------------
pub fn authorize_tournament_session_ix(
    program_id: Pubkey,
    tournament_id: u64,
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
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[b"tournament_session", &tournament_id.to_le_bytes(), player.as_ref()],
        &program_id,
    ).0;

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
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
            AccountMeta::new(session_delegation_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// session_create_game
// ---------------------------------------------------------------------------
pub fn session_create_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    game_id: u64,
    white_session: Pubkey,
    white_player: Pubkey,
    wager: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[b"tournament_session", &tournament_id.to_le_bytes(), white_player.as_ref()],
        &program_id,
    ).0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("session_create_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager.to_le_bytes());
    // MatchType::Free = 0
    data.push(0);
    // Country string (empty for benchmark)
    data.extend_from_slice(&(0u32).to_le_bytes());
    data.extend_from_slice(&600u64.to_le_bytes()); // base_time_seconds
    data.extend_from_slice(&0u16.to_le_bytes()); // increment_seconds

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
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

// ---------------------------------------------------------------------------
// session_join_game
// ---------------------------------------------------------------------------
pub fn session_join_game_ix(
    program_id: Pubkey,
    tournament_id: u64,
    game_id: u64,
    session_key: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[b"tournament_session", &tournament_id.to_le_bytes(), player.as_ref()],
        &program_id,
    ).0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    ).0;
    let white_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, white_player.as_ref()],
        &program_id,
    ).0;
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("session_join_game").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
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

// ---------------------------------------------------------------------------
// record_swiss_result
// ---------------------------------------------------------------------------
pub fn record_swiss_result_ix(
    program_id: Pubkey,
    tournament_id: u64,
    round: u8,
    board: u16,
    result_variant: u8, // 0 = Win, 1 = Loss, 2 = Draw (SwissMatchResult)
    player: Pubkey,
    opponent: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_0 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_1 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_2 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;
    let tournament_players_shard_3 = Pubkey::find_program_address(
        &[TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("record_swiss_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.push(round);
    data.extend_from_slice(&board.to_le_bytes());
    data.push(result_variant);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(tournament_players_shard_0, false),
            AccountMeta::new(tournament_players_shard_1, false),
            AccountMeta::new(tournament_players_shard_2, false),
            AccountMeta::new(tournament_players_shard_3, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(opponent, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// claim_prize (1v1 wager payout)
// ---------------------------------------------------------------------------
pub fn claim_prize_ix(
    program_id: Pubkey,
    game_id: u64,
    winner: Pubkey,
    fee_payer: Pubkey,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let data = anchor_discriminator("claim_prize").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(winner, true),
            AccountMeta::new(fee_payer, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// close_tournament (distributes prizes, marks Closed)
// prize_recipients are passed as remaining_accounts in prize-place order
// ---------------------------------------------------------------------------
pub fn close_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    prize_recipients: &[Pubkey],
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()], &program_id,
    ).0;
    let prize_escrow_pda = Pubkey::find_program_address(
        &[TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()], &program_id,
    ).0;
    let treasury_vault = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id).0;

    let mut data = anchor_discriminator("close_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new(prize_escrow_pda, false),
        AccountMeta::new(treasury_vault, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new(authority, true),
    ];
    for recipient in prize_recipients {
        accounts.push(AccountMeta::new(*recipient, false));
    }

    Ok(Instruction { program_id, accounts, data })
}

// ---------------------------------------------------------------------------
// resign
// ---------------------------------------------------------------------------
pub fn resign_game_ix(
    program_id: Pubkey,
    game_id: u64,
    player: Pubkey,
    white_pubkey: Pubkey,
    black_pubkey: Pubkey,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("resign").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(white_pubkey, false),
            AccountMeta::new(black_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}
