//! Solana instruction builders
//!
//! Anchor-compatible instruction builders that mirror the on-chain
//! `xfchess-game` program accounts and instruction arguments.
//!
//! Reference: programs/xfchess-game/src/lib.rs

use anyhow::Result;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};


/// Deployed program ID (must match `declare_id!` in xfchess-game).
pub const PROGRAM_ID: &str = "C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf";

/// PDA seeds — kept in sync with `programs/xfchess-game/src/constants.rs`.
pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub const USERNAME_SEED: &[u8] = b"username";

/// Compute the 8-byte Anchor discriminator for `global:<fn_name>`.
fn anchor_discriminator(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Encode a Borsh-style `String` (u32 length prefix + utf-8 bytes).
fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

/// Encode a Borsh-style `Option<String>`.
#[allow(dead_code)]
fn borsh_option_string(opt: &Option<String>) -> Vec<u8> {
    match opt {
        Some(s) => {
            let mut buf = vec![1u8];
            buf.extend(borsh_string(s));
            buf
        }
        None => vec![0u8],
    }
}

// ---------------------------------------------------------------------------
// create_game
// ---------------------------------------------------------------------------

/// Build a `create_game` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn create_game(ctx, game_id: u64, wager_amount: u64, match_type: MatchType, country: String, base_time_seconds: u64, increment_seconds: u16)
/// ```
///
/// `match_type` encoding: Free=0, Ranked=1, Wager=2.
/// `fee_payer` is the VPS session key — must co-sign the transaction.
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
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let move_log_pda = Pubkey::find_program_address(
        &[MOVE_LOG_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("create_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    // MatchType Anchor enum: Free=0, Ranked=1, Wager=2
    data.push(match_type);
    // country: Borsh String (u32 len prefix + utf-8 bytes)
    data.extend_from_slice(&(country.len() as u32).to_le_bytes());
    data.extend_from_slice(country.as_bytes());
    // base_time_seconds: u64 LE, increment_seconds: u16 LE
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
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

/// Build a `join_game` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn join_game(ctx, game_id: u64)
/// ```
///
/// `white_player` is the pubkey stored in `game.white` (read from chain before calling).
/// `fee_payer` must match `game.fee_payer` (the VPS session key set during create_game).
pub fn join_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    white_player: Pubkey,
    fee_payer: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let white_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, white_player.as_ref()],
        &program_id,
    )
    .0;

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

/// Build a `record_move` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn record_move(ctx, game_id: u64, move_str: String, next_fen: String)
/// ```
///
/// `annotation`, `move_time`, and `prev_hash` are client-side metadata used
/// for local hash-chaining and display; they are **not** sent on-chain.
pub fn record_move_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
    move_str: String,
    next_fen: String,
    _annotation: Option<String>,
    _move_time: Option<String>,
    _prev_hash: &[u8; 32],
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let move_log_pda = Pubkey::find_program_address(
        &[MOVE_LOG_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend(borsh_string(&move_str));
    data.extend(borsh_string(&next_fen));

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new_readonly(player, true),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// finalize_game
// ---------------------------------------------------------------------------

/// Build a `finalize_game` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn finalize_game(ctx, game_id: u64, result: GameResult)
/// ```
///
/// `result` encoding (Anchor enum):
///   0 → `GameResult::None`
///   1 + 32-byte pubkey → `GameResult::Winner(Pubkey)`
///   2 → `GameResult::Draw`
pub fn finalize_game_ix(
    program_id: Pubkey,
    payer: Pubkey,
    game_id: u64,
    result_code: u8,
    white_pubkey: Pubkey,
    black_pubkey: Pubkey,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let white_profile = Pubkey::find_program_address(
        &[PROFILE_SEED, white_pubkey.as_ref()],
        &program_id,
    )
    .0;
    let black_profile = Pubkey::find_program_address(
        &[PROFILE_SEED, black_pubkey.as_ref()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    // Encode GameResult as Anchor enum
    match result_code {
        0 => {
            // WhiteWins → GameResult::Winner(white_pubkey)
            data.push(1);
            data.extend_from_slice(white_pubkey.as_ref());
        }
        1 => {
            // BlackWins → GameResult::Winner(black_pubkey)
            data.push(1);
            data.extend_from_slice(black_pubkey.as_ref());
        }
        _ => {
            // Draw → GameResult::Draw
            data.push(2);
        }
    }

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(white_pubkey, false),
            AccountMeta::new(black_pubkey, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// init_profile
// ---------------------------------------------------------------------------

/// Build an `init_profile` instruction.
pub fn init_profile_ix(
    program_id: Pubkey,
    player: Pubkey,
    username: String,
    country: String,
) -> Result<Instruction> {
    let profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    ).0;
    let username_record_pda = Pubkey::find_program_address(
        &[USERNAME_SEED, username.as_bytes()],
        &program_id,
    ).0;

    let mut data = anchor_discriminator("init_profile").to_vec();
    data.extend(borsh_string(&username));
    data.extend(borsh_string(&country));

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(profile_pda, false),
            AccountMeta::new(username_record_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

// ---------------------------------------------------------------------------
// authorize_session_key
// ---------------------------------------------------------------------------

/// Build an `authorize_session_key` instruction.
pub fn authorize_session_key_ix(
    payer: Pubkey,
    game_pda: Pubkey,
    session_key: Pubkey,
    expires_at: i64,
) -> Result<Instruction> {
    let program_id: Pubkey = PROGRAM_ID.parse()?;

    let mut data = anchor_discriminator("authorize_session_key").to_vec();
    data.extend_from_slice(session_key.as_ref());
    data.extend_from_slice(&expires_at.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(session_key, false),
        ],
        data,
    })
}

/// Re-export program ID for convenience.
pub fn get_program_id() -> Result<Pubkey> {
    PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid program ID: {}", e))
}
