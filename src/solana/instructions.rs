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

pub use xfchess_game::state::game::GameType;

/// Deployed program ID (must match `declare_id!` in xfchess-game).
pub const PROGRAM_ID: &str = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP";

/// PDA seeds — kept in sync with `programs/xfchess-game/src/constants.rs`.
pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";

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
/// pub fn create_game(ctx, game_id: u64, wager_amount: u64, game_type: GameType)
/// ```
pub fn create_game_ix(
    program_id: Pubkey,
    payer: Pubkey,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
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
    // GameType is a single-byte Anchor enum: PvP = 0, PvAI = 1
    data.push(match game_type {
        GameType::PvP => 0,
        GameType::PvAI => 1,
    });

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(payer, true),
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
pub fn join_game_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let escrow_pda = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("join_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(player, true),
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
// commit_move_batch  (used by rollup_network_bridge)
// ---------------------------------------------------------------------------

/// Build a `commit_move_batch` instruction.
pub fn commit_move_batch_ix(
    payer: Pubkey,
    game_pda: Pubkey,
    moves: Vec<(u8, u8)>,
    _signatures: Vec<[u8; 64]>,
) -> Result<Instruction> {
    let program_id: Pubkey = PROGRAM_ID.parse()?;

    let mut data = anchor_discriminator("commit_move_batch").to_vec();
    data.extend_from_slice(&(moves.len() as u16).to_le_bytes());
    for (from, to) in moves {
        data.push(from);
        data.push(to);
    }

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(game_pda, false),
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
