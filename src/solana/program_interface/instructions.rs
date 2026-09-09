use anyhow::Result;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
#[allow(deprecated)]
use solana_system_interface::program as system_program;

pub const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const USERNAME_SEED: &[u8] = b"username";
pub const FRIENDSHIP_SEED: &[u8] = b"friendship";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";
pub const TOURNAMENT_PLAYERS_SEED: &[u8] = b"tourney_players";
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

pub fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

pub fn bracket_position(max_players: u16, match_index: u16) -> (u8, Option<u16>, u8) {
    let total_matches = max_players.saturating_sub(1);
    let mut round_start = 0u16;
    let mut round_size = max_players / 2;
    let mut round = 0u8;
    while round_size > 1 && match_index >= round_start + round_size {
        round_start += round_size;
        round_size /= 2;
        round += 1;
    }
    let pos_in_round = match_index - round_start;
    let next = if match_index + 1 >= total_matches {
        None // the final
    } else {
        Some(round_start + round_size + pos_in_round / 2)
    };
    (round, next, (pos_in_round % 2) as u8)
}

fn shard_meta(program_id: &Pubkey, tournament_id: u64, idx: u8, max_players: u16) -> AccountMeta {
    if idx < required_shards(max_players) {
        let pda = Pubkey::find_program_address(
            &[
                TOURNAMENT_PLAYERS_SEED,
                &[idx],
                &tournament_id.to_le_bytes(),
            ],
            program_id,
        )
        .0;
        AccountMeta::new(pda, false)
    } else {
        AccountMeta::new_readonly(*program_id, false)
    }
}

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
// create_game
// ---------------------------------------------------------------------------

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

    let mut data = anchor_discriminator("create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    // MatchType Anchor enum: Free=0, Ranked=1, Wager=2
    data.push(match_type);
    // platform_fee: u64 LE (replaces country String — universal fee from live price feed)
    data.extend_from_slice(&platform_fee.to_le_bytes());
    // base_time_seconds: u64 LE, increment_seconds: u16 LE
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

// ---------------------------------------------------------------------------
// cancel_game
// ---------------------------------------------------------------------------

pub fn cancel_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    white: Pubkey,
    black: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("cancel_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(white, false),
            AccountMeta::new(black, false),
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
            AccountMeta::new(session_pda, false),
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
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// offer_draw / accept_draw
// ---------------------------------------------------------------------------

pub fn offer_draw_ix(program_id: Pubkey, game_id: u64, player: Pubkey) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("offer_draw").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player, true),
        ],
        data,
    })
}

pub fn accept_draw_ix(program_id: Pubkey, game_id: u64, player: Pubkey) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("accept_draw").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(player, true),
        ],
        data,
    })
}

pub fn claim_timeout_ix(program_id: Pubkey, game_id: u64, caller: Pubkey) -> Result<Instruction> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = anchor_discriminator("claim_timeout").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(caller, true),
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

pub fn verify_profile_ix(program_id: Pubkey, admin: Pubkey, player: Pubkey) -> Result<Instruction> {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let data = anchor_discriminator("verify_profile").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(admin, true), // The KYC authority fee-payer
            AccountMeta::new_readonly(player, false),
        ],
        data,
    })
}

pub fn set_username_ix(program_id: Pubkey, player: Pubkey, username: &str) -> Result<Instruction> {
    let player_profile_pda =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &program_id).0;

    let username_record_pda =
        Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id).0;

    let mut data = anchor_discriminator("set_username").to_vec();
    data.extend(borsh_string(username));

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(username_record_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(player, false), // authority = player
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn initialize_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    max_players: u16,
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
        &[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let prize_shares: [u16; 10] = if max_players <= 2 {
        [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0] // head-to-head: no 3rd place
    } else {
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]
    };

    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());
    data.push(1); // TournamentType::SingleElimination (borsh variant 1; Swiss { rounds } is 0)
    data.extend_from_slice(&0u32.to_le_bytes()); // elo_min
    data.extend_from_slice(&u32::MAX.to_le_bytes()); // elo_max
    data.extend_from_slice(&max_players.to_le_bytes()); // min_players
    for share in prize_shares {
        data.extend_from_slice(&share.to_le_bytes());
    }
    data.extend_from_slice(&0u64.to_le_bytes()); // platform_fee
    data.push(0); // winner_takes_all = false
    data.extend_from_slice(host_treasury.as_ref());
    data.push(0); // usdc_mint = None
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    let token_program: Pubkey = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".parse()?;
    let associated_token_program: Pubkey =
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse()?;

    Ok(Instruction {
        program_id,
        // Account order must match `InitializeTournament`; the two optional
        // USDC accounts are passed as the program ID (Anchor's `None` marker).
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(usdc_prize_escrow_authority, false),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new_readonly(program_id, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_token_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn initialize_escrow_ix(
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

pub fn initialize_shards_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let shard = |idx: u8| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_PLAYERS_SEED,
                &[idx],
                &tournament_id.to_le_bytes(),
            ],
            &program_id,
        )
        .0
    };

    let (name, shard_count) = if max_players <= 64 {
        ("initialize_shards_small", 1u8)
    } else if max_players <= 128 {
        ("initialize_shards_medium", 2)
    } else {
        ("initialize_shards_large", 4)
    };

    let mut data = anchor_discriminator(name).to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let mut accounts = vec![AccountMeta::new_readonly(tournament_pda, false)];
    for idx in 0..shard_count {
        accounts.push(AccountMeta::new(shard(idx), false));
    }
    accounts.push(AccountMeta::new(authority, true));
    accounts.push(AccountMeta::new_readonly(system_program::id(), false));

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

pub fn fund_sol_prize_ix(
    program_id: Pubkey,
    operator: Pubkey,
    tournament_id: u64,
    amount_lamports: u64,
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

    let mut data = anchor_discriminator("fund_sol_prize").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&amount_lamports.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn register_player_ix(
    program_id: Pubkey,
    player: Pubkey,
    tournament_id: u64,
    max_players: u16,
    host_treasury: Pubkey,
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

    Ok(Instruction {
        program_id,
        // Account order must match `RegisterPlayer`: tournament, profile,
        // player, escrow, shards 0-3, host_treasury, system_program.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(
                Pubkey::find_program_address(
                    &[
                        TOURNAMENT_PLAYERS_SEED,
                        &[0u8],
                        &tournament_id.to_le_bytes(),
                    ],
                    &program_id,
                )
                .0,
                false,
            ),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(host_treasury, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn start_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    max_players: u16,
    host_treasury: Pubkey,
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
    let shard_0 = Pubkey::find_program_address(
        &[
            TOURNAMENT_PLAYERS_SEED,
            &[0u8],
            &tournament_id.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        // Account order must match `StartTournament`: tournament, shards 0-3,
        // escrow, host_treasury, authority, system_program.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_0, false),
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

#[allow(clippy::too_many_arguments)]
pub fn initialize_match_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    match_index: u16,
    round: u8,
    player_white: Option<Pubkey>,
    player_black: Option<Pubkey>,
    next_match_for_winner: Option<u16>,
    next_match_slot: u8,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("initialize_match").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.push(round);
    for player in [player_white, player_black] {
        match player {
            Some(pk) => {
                data.push(1);
                data.extend_from_slice(pk.as_ref());
            }
            None => data.push(0),
        }
    }
    match next_match_for_winner {
        Some(n) => {
            data.push(1);
            data.extend_from_slice(&n.to_le_bytes());
        }
        None => data.push(0),
    }
    data.push(next_match_slot);

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(match_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn record_match_result_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    match_index: u16,
    winner: Pubkey,
    loser: Pubkey,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let match_pda = Pubkey::find_program_address(
        &[
            TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_match_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&match_index.to_le_bytes());
    data.extend_from_slice(winner.as_ref());
    data.extend_from_slice(loser.as_ref());

    Ok(Instruction {
        program_id,
        // Account order must match `RecordMatchResult`: tournament, match, authority.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(match_pda, false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn record_swiss_result_ix(
    program_id: Pubkey,
    player: Pubkey,
    opponent: Pubkey,
    tournament_id: u64,
    max_players: u16,
    round: u8,
    board: u16,
    result: u8,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let shard_0 = Pubkey::find_program_address(
        &[
            TOURNAMENT_PLAYERS_SEED,
            &[0u8],
            &tournament_id.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_swiss_result").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.push(round);
    data.extend_from_slice(&board.to_le_bytes());
    data.push(result); // SwissMatchResult Borsh variant tag

    Ok(Instruction {
        program_id,
        // Account order must match `RecordSwissResult`: tournament, shards 0-3,
        // player (signer), opponent, system_program.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_0, false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(opponent, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn advance_winner_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    source_match_index: u16,
    target_match_index: u16,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let match_pda = |idx: u16| {
        Pubkey::find_program_address(
            &[
                TOURNAMENT_MATCH_SEED,
                &tournament_id.to_le_bytes(),
                &idx.to_le_bytes(),
            ],
            &program_id,
        )
        .0
    };

    let mut data = anchor_discriminator("advance_winner").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&source_match_index.to_le_bytes());
    data.extend_from_slice(&target_match_index.to_le_bytes());

    Ok(Instruction {
        program_id,
        // Account order must match `AdvanceWinner`: tournament, source, target, authority.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(match_pda(source_match_index), false),
            AccountMeta::new(match_pda(target_match_index), false),
            AccountMeta::new(authority, true),
        ],
        data,
    })
}

pub fn leave_tournament_ix(
    program_id: Pubkey,
    player: Pubkey,
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
    let shard_0 = Pubkey::find_program_address(
        &[
            TOURNAMENT_PLAYERS_SEED,
            &[0u8],
            &tournament_id.to_le_bytes(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("leave_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        // Account order must match `LeaveTournament`: tournament, shards 0-3,
        // player, escrow, system_program.
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(shard_0, false),
            shard_meta(&program_id, tournament_id, 1, max_players),
            shard_meta(&program_id, tournament_id, 2, max_players),
            shard_meta(&program_id, tournament_id, 3, max_players),
            AccountMeta::new(player, true),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

pub fn get_program_id() -> Result<Pubkey> {
    PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid program ID: {}", e))
}

// ── Solana Friends ────────────────────────────────────────────────────────────
//
// Mirrors `programs/xfchess-game/src/account_ix/friends_ix.rs`. The `Friendship`
// PDA is addressed by the two wallets in canonical (sorted) order, so both sides
// derive the same account. Callers pass any two wallets; we sort them here.

fn friendship_pair(a: Pubkey, b: Pubkey, program_id: &Pubkey) -> (Pubkey, Pubkey, Pubkey) {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    let pda =
        Pubkey::find_program_address(&[FRIENDSHIP_SEED, lo.as_ref(), hi.as_ref()], program_id).0;
    (lo, hi, pda)
}

pub fn send_friend_request_ix(
    program_id: Pubkey,
    requester: Pubkey,
    other: Pubkey,
) -> Result<Instruction> {
    let (lo, hi, friendship) = friendship_pair(requester, other, &program_id);
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(friendship, false),
            AccountMeta::new_readonly(lo, false),
            AccountMeta::new_readonly(hi, false),
            AccountMeta::new(requester, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: anchor_discriminator("send_friend_request").to_vec(),
    })
}

pub fn accept_friend_request_ix(
    program_id: Pubkey,
    addressee: Pubkey,
    other: Pubkey,
) -> Result<Instruction> {
    let (lo, hi, friendship) = friendship_pair(addressee, other, &program_id);
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(friendship, false),
            AccountMeta::new_readonly(lo, false),
            AccountMeta::new_readonly(hi, false),
            AccountMeta::new_readonly(addressee, true),
        ],
        data: anchor_discriminator("accept_friend_request").to_vec(),
    })
}

pub fn close_friendship_ix(
    program_id: Pubkey,
    signer: Pubkey,
    other: Pubkey,
) -> Result<Instruction> {
    let (lo, hi, friendship) = friendship_pair(signer, other, &program_id);
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(friendship, false),
            AccountMeta::new_readonly(lo, false),
            AccountMeta::new_readonly(hi, false),
            AccountMeta::new(signer, true),
        ],
        data: anchor_discriminator("close_friendship").to_vec(),
    })
}

pub fn block_user_ix(program_id: Pubkey, signer: Pubkey, other: Pubkey) -> Result<Instruction> {
    let (lo, hi, friendship) = friendship_pair(signer, other, &program_id);
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(friendship, false),
            AccountMeta::new_readonly(lo, false),
            AccountMeta::new_readonly(hi, false),
            AccountMeta::new_readonly(signer, true),
        ],
        data: anchor_discriminator("block_user").to_vec(),
    })
}
