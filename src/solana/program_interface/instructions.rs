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
};
#[allow(deprecated)]
use solana_sdk::system_program;

pub use xfchess_game::state::game::GameType;

/// Deployed program ID (must match `declare_id!` in xfchess-game).
pub const PROGRAM_ID: &str = "C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf";

/// PDA seeds — kept in sync with `programs/xfchess-game/src/constants.rs`.
pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
#[allow(dead_code)]
pub const USERNAME_SEED: &[u8] = b"username";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
#[allow(dead_code)]
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
#[allow(dead_code)]
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
#[allow(dead_code)]
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
#[allow(dead_code)]
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";

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
#[allow(dead_code)]
fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
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
#[allow(dead_code)]
pub fn record_move_ix(
    program_id: Pubkey,
    player: Pubkey,
    player_wallet: Pubkey, // Base wallet for session delegation PDA
    game_id: u64,
    move_str: String,
    next_fen: String,
    nonce: u64,
    signature: Option<Vec<u8>>,
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
    let session_delegation_pda = Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            player_wallet.as_ref(),
        ],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend(borsh_string(&move_str));
    data.extend(borsh_string(&next_fen));
    data.extend_from_slice(&nonce.to_le_bytes());
    
    // Borsh Option<Vec<u8>> encoding:
    // 0 -> None
    // 1 -> Some(...)
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
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(session_delegation_pda, false),
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
// authorize_session_key
// ---------------------------------------------------------------------------

/// Build an `authorize_session_key` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn authorize_session_key(ctx, game_id: u64, session_pubkey: Pubkey)
/// ```
///
/// Accounts (order matches `AuthorizeSessionCtx`):
///   0. game                (mut, seeds=["game", game_id])
///   1. session_delegation  (init, seeds=["session_delegation", game_id, player])
///   2. player              (mut, signer)
///   3. system_program
pub fn authorize_session_key_ix(
    program_id: Pubkey,
    player: Pubkey,
    game_id: u64,
    session_pubkey: Pubkey,
    duration_seconds: i64,
) -> Result<Instruction> {
    let game_pda = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[SESSION_DELEGATION_SEED, &game_id.to_le_bytes(), player.as_ref()],
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

/// Build an `init_profile` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn init_profile(ctx)
/// ```
///
/// Must be called once per player before `finalize_game` (which reads both
/// `white_profile` and `black_profile`). Safe to skip if the PDA already exists.
///
/// Accounts:
///   0. player_profile  (init, seeds=["profile", player])
///   1. player          (mut, signer)
///   2. system_program
#[allow(dead_code)]
pub fn init_profile_ix(program_id: Pubkey, player: Pubkey) -> Result<Instruction> {
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    )
    .0;

    let data = anchor_discriminator("init_profile").to_vec();

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Build a `verify_profile` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn verify_profile(ctx)
/// ```
///
/// Accounts:
///   0. player_profile  (mut, seeds=["profile", player])
///   1. admin           (mut, signer)
///   2. player          (pubkey only)
#[allow(dead_code)]
pub fn verify_profile_ix(program_id: Pubkey, admin: Pubkey, player: Pubkey) -> Result<Instruction> {
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    )
    .0;

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

/// Build a `set_username` instruction.
///
/// On-chain signature:
/// ```ignore
/// pub fn set_username(ctx, username: String)
/// ```
///
/// Accounts:
///   0. player_profile  (mut, seeds=["profile", player])
///   1. username_record (init, seeds=["username", username.as_bytes()])
///   2. player          (mut, signer)
///   3. authority       (must match profile.authority)
///   4. system_program
#[allow(dead_code)]
pub fn set_username_ix(
    program_id: Pubkey,
    player: Pubkey,
    username: &str,
) -> Result<Instruction> {
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    )
    .0;

    let username_record_pda = Pubkey::find_program_address(
        &[USERNAME_SEED, username.as_bytes()],
        &program_id,
    )
    .0;

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

/// Build an `initialize_tournament` instruction.
#[allow(dead_code)]
pub fn initialize_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
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

    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Build a `register_player` instruction.
#[allow(dead_code)]
pub fn register_player_ix(
    program_id: Pubkey,
    player: Pubkey,
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
    let player_profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, player.as_ref()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("register_player").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(player_profile_pda, false),
            AccountMeta::new(player, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Build a `start_tournament` instruction.
#[allow(dead_code)]
pub fn start_tournament_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("start_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Build a `record_match_result` instruction.
#[allow(dead_code)]
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
    )
    .0;

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

/// Build an `advance_final` instruction.
#[allow(dead_code)]
pub fn advance_final_ix(
    program_id: Pubkey,
    authority: Pubkey,
    tournament_id: u64,
) -> Result<Instruction> {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;

    let mut data = anchor_discriminator("advance_final").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    })
}

/// Re-export program ID for convenience.
#[allow(dead_code)]
pub fn get_program_id() -> Result<Pubkey> {
    PROGRAM_ID
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid program ID: {}", e))
}
