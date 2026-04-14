//! Solana instruction builders and RPC helpers for the XFChess program.
//!
//! This module provides functions to build Solana instructions for:
//! - Recording chess moves on the Execution Rollup
//! - Undelegating games (committing ER state back to devnet)
//! - Finalizing games (setting winner, paying out escrow)
//! - Verifying player profiles (KYC)
//!
//! Also provides RPC client helpers for signing and submitting transactions
//! to both devnet and the MagicBlock Execution Rollup.

use anyhow::{anyhow, Result};
use tracing::warn;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
#[allow(deprecated)]
use solana_sdk::system_instruction;

/// PDA seed for game accounts
pub const GAME_SEED: &[u8] = b"game";
/// PDA seed for move log accounts
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
/// PDA seed for session delegation accounts
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
/// PDA seed for player profile accounts
pub const PROFILE_SEED: &[u8] = b"profile";
/// PDA seed for wager escrow accounts
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";

/// MagicBlock magic context account (ER-only)
const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";
/// MagicBlock magic program (ER-only)
const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";

/// Computes the Anchor discriminator for a given instruction name.
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    hasher.finalize()[..8].try_into().unwrap()
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
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;
    let session_delegation_pda = Pubkey::find_program_address(
        &[SESSION_DELEGATION_SEED, &game_id.to_le_bytes(), wallet_pubkey.as_ref()],
        program_id,
    ).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY.parse().expect("magic context pubkey");
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY.parse().expect("magic program pubkey");

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

    Instruction {
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
    }
}

/// Builds an `undelegate_game` instruction for the ER.
///
/// Commits the ER state (game + move_log) back to devnet and releases the accounts.
pub fn undelegate_game_ix(program_id: &Pubkey, session_pubkey: &Pubkey, game_id: u64) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda = Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY.parse().expect("magic context pubkey");
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY.parse().expect("magic program pubkey");

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(*session_pubkey, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    }
}

/// Builds a `finalize_game` instruction for devnet.
///
/// Sets game.status = Finished, pays out the wager escrow, and updates ELO.
/// Winner: Some("white") | Some("black") | None (draw).
pub fn finalize_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    winner: Option<&str>,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let white_profile = Pubkey::find_program_address(&[PROFILE_SEED, white.as_ref()], program_id).0;
    let black_profile = Pubkey::find_program_address(&[PROFILE_SEED, black.as_ref()], program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0;

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

/// Funds `dest` with `lamports` from `payer`, submit to `rpc_url`.
pub fn fund_account(
    rpc: &RpcClient,
    payer: &Keypair,
    dest: &Pubkey,
    lamports: u64,
) -> Result<Signature> {
    let ix = system_instruction::transfer(&payer.pubkey(), dest, lamports);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    Ok(rpc.send_and_confirm_transaction(&tx)?)
}

/// Signs `ix` with `signer` (fee-payer = signer) and submits to `rpc_url`.
pub fn sign_and_submit(rpc: &RpcClient, signer: &Keypair, instructions: &[Instruction]) -> Result<Signature> {
    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::confirmed(),
    )
    .map_err(|e| anyhow!(e))
}

/// Signs and submits to the MagicBlock ER with `skip_preflight = true`.
///
/// The ER's preflight simulator may reject transactions with -32003
/// ("Attempt to load a program that does not exist") because the XFChess
/// program is not in its preflight cache. Skipping preflight lets the TX land.
/// After sending, we poll for confirmation (ER confirms in sub-second to ~5 s).
pub fn sign_and_submit_er(rpc: &RpcClient, signer: &Keypair, instructions: &[Instruction]) -> Result<Signature> {
    use solana_client::rpc_config::RpcSendTransactionConfig;
    use std::time::{Duration, Instant};

    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);

    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(&tx, config)
        .map_err(|e| anyhow!(e))?;

    let commitment = CommitmentConfig::confirmed();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err(anyhow!("ER record_move confirmation timeout for {sig}"));
        }
        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(anyhow!("ER record_move failed: {e:?}")),
            Ok(None) => std::thread::sleep(Duration::from_millis(400)),
            Err(e) => {
                warn!("[ER] poll error (non-fatal): {e}");
                std::thread::sleep(Duration::from_millis(400));
            }
        }
    }
}

/// Submits an already-signed serialized transaction.
///
/// Used for wallet-signed setup TXs. Accepts both legacy `Transaction`
/// and `VersionedTransaction` (v0). Uses `confirmed` commitment.
pub fn submit_signed_tx(rpc: &RpcClient, tx_bytes: &[u8]) -> Result<Signature> {
    let tx: VersionedTransaction = bincode::deserialize(tx_bytes).map_err(|e| anyhow!(e))?;
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::confirmed(),
    )
    .map_err(|e| anyhow!(e))
}

/// Creates an RPC client with confirmed commitment.
pub fn make_rpc(url: &str) -> RpcClient {
    RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed())
}
