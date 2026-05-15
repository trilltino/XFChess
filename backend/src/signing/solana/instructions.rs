//! Solana instruction builders for XFChess program instructions.

use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::{GAME_SEED, MAGIC_CONTEXT_PUBKEY, MAGIC_PROGRAM_PUBKEY, MOVE_LOG_SEED, PLATFORM_FEE_VAULT_SEED, PROFILE_SEED, SESSION_DELEGATION_SEED, TOURNAMENT_SEED, WAGER_ESCROW_SEED};

/// Computes the Anchor discriminator for a given instruction name.
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    hasher.finalize()[..8].try_into().expect("SHA256 hash should be at least 8 bytes")
}

/// Borsh-encodes a string (length prefix + bytes).
fn borsh_string(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

/// Builds a `record_move` instruction for the Execution Rollup.
///
/// Records a chess move on the ER with optional signature for replay protection.
pub fn record_move_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    wallet_pubkey: &Pubkey,
    game_id: u64,
    move_str: &str,
    next_fen: &str,
    nonce: u64,
    signature: Option<Vec<u8>>,
) -> anyhow::Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[SESSION_DELEGATION_SEED, &game_id.to_le_bytes(), wallet_pubkey.as_ref()],
        program_id,
    ).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY.parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic context pubkey: {}", e))?;
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY.parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic program pubkey: {}", e))?;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend(borsh_string(move_str));
    data.extend(borsh_string(next_fen));
    data.extend_from_slice(&nonce.to_le_bytes());

    // Optional Vec<u8> (Borsh encoding)
    if let Some(sig) = signature {
        data.push(1); // Some
        data.extend_from_slice(&(sig.len() as u32).to_le_bytes());
        data.extend_from_slice(&sig);
    } else {
        data.push(0); // None
    }

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new_readonly(*session_pubkey, true),
            AccountMeta::new_readonly(session_delegation_pda, false),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

/// Builds an `undelegate_game` instruction for the ER.
///
/// Commits the ER state (game + move_log) back to devnet and releases the accounts.
pub fn undelegate_game_ix(program_id: &Pubkey, session_pubkey: &Pubkey, game_id: u64) -> anyhow::Result<Instruction> {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda = Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY.parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic context pubkey: {}", e))?;
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY.parse()
        .map_err(|e| anyhow::anyhow!("Invalid magic program pubkey: {}", e))?;

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(*session_pubkey, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    })
}

/// Builds a `finalize_game` instruction for devnet.
///
/// Sets game.status = Finished, pays out the wager escrow, and updates ELO.
/// Winner: Some("white") | Some("black") | None (draw).
///
/// `fee_payer` is the ephemeral rollups relayer pubkey that gets reimbursed from escrow.
pub fn finalize_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    winner: Option<&str>,
    fee_payer: &Pubkey,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let white_profile = Pubkey::find_program_address(&[PROFILE_SEED, white.as_ref()], program_id).0;
    let black_profile = Pubkey::find_program_address(&[PROFILE_SEED, black.as_ref()], program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0;
    let treasury_vault = Pubkey::find_program_address(&[b"treasury_vault"], program_id).0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    // GameResult Borsh encoding: 1 = Winner(Pubkey), 2 = Draw
    match winner {
        Some("white") => { data.push(1); data.extend_from_slice(white.as_ref()); }
        Some("black") => { data.push(1); data.extend_from_slice(black.as_ref()); }
        _ =>            { data.push(2); }
    }

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(*white, false),
            AccountMeta::new(*black, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(treasury_vault, false),
            AccountMeta::new(*fee_payer, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}

/// Builds a `verify_profile` instruction for devnet.
///
/// Marks a player as KYC-verified on-chain.
pub fn verify_profile_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    player: &Pubkey,
) -> Instruction {
    let player_profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], program_id).0;

    let data = anchor_discriminator("verify_profile").to_vec();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(player_profile_pda, false),
            AccountMeta::new(*admin, true), // The KYC authority fee-payer
            AccountMeta::new_readonly(*player, false),
        ],
        data,
    }
}

/// Builds a `claim_fees` instruction for the platform fee vault.
///
/// Transfers accumulated fees from the PlatformFeeVault to the host wallet.
/// This instruction is permissionless - anyone can trigger it.
///
/// # Arguments
/// * `program_id` - The XFChess program ID
/// * `caller` - The account triggering the claim (fee-payer)
/// * `host_wallet` - The wallet that receives the claimed fees (must match vault.host_wallet)
pub fn claim_fees_ix(
    program_id: &Pubkey,
    caller: &Pubkey,
    host_wallet: &Pubkey,
) -> Instruction {
    let fee_vault_pda = Pubkey::find_program_address(&[PLATFORM_FEE_VAULT_SEED], program_id).0;

    let data = anchor_discriminator("claim_fees").to_vec();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*caller, true),
            AccountMeta::new(fee_vault_pda, false),
            AccountMeta::new(*host_wallet, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}

/// Builds a `leave_tournament` instruction for devnet.
///
/// Removes a player from the tournament and triggers a refund.
pub fn leave_tournament_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    player: &Pubkey,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        program_id,
    ).0;
 
    let data = anchor_discriminator("leave_tournament").to_vec();
 
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(*player, true),
            AccountMeta::new(*host_treasury, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}
 
/// Builds an `initialize_tournament` instruction for devnet.
pub fn initialize_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    tournament_id: u64,
    name: &str,
    entry_fee: u64,
    platform_fee: u64,
    max_players: u16,
    tournament_type: u8, // 0 = SingleElimination, 1 = Swiss
    swiss_rounds: u8,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    winner_takes_all: bool,
    host_treasury: &Pubkey,
) -> Instruction {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        program_id,
    ).0;
 
    let mut data = anchor_discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&max_players.to_le_bytes());
    
    // TournamentType Borsh encoding
    data.push(tournament_type);
    if tournament_type == 1 { // Swiss
        data.push(swiss_rounds);
    }
 
    data.extend_from_slice(&elo_min.to_le_bytes());
    data.extend_from_slice(&elo_max.to_le_bytes());
    data.extend_from_slice(&min_players.to_le_bytes());
    for &share in prize_shares.iter() {
        data.extend_from_slice(&share.to_le_bytes());
    }
    data.extend_from_slice(&platform_fee.to_le_bytes());
    data.push(if winner_takes_all { 1 } else { 0 });
    data.extend_from_slice(host_treasury.as_ref());
    
    // Optional usdc_mint (None = 0)
    data.push(0); 
    
    // Default time controls
    data.extend_from_slice(&600u64.to_le_bytes()); // 10 mins
    data.extend_from_slice(&0u16.to_le_bytes()); // 0 inc
 
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(*admin, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ],
        data,
    }
}
