//! Session key authorization on-chain.
//!
//! This module handles on-chain session key authorization via the CreateSession instruction.
//! This authorizes the session key to sign transactions on behalf of the wallet.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::Instruction,
};
use solana_sdk::system_program;
use anyhow::Result;

/// Program ID for XFChess
pub const PROGRAM_ID: &str = "C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf";

/// Creates a session key authorization instruction.
///
/// # Arguments
/// * `payer` - The wallet paying for the transaction
/// * `session_key` - The session key to authorize
/// * `duration_hours` - Session duration in hours (default 24)
/// * `spending_limit` - Max lamports the session can spend (default 0.5 SOL)
/// * `max_wager` - Max lamports per wager (default 10 SOL)
///
/// # Returns
/// The CreateSession instruction
pub fn create_session_instruction(
    payer: &Pubkey,
    session_key: &Pubkey,
    duration_hours: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
) -> Instruction {
    use sha2::{Digest, Sha256};
    
    // Compute Anchor discriminator for "create_session"
    let mut hasher = Sha256::new();
    hasher.update(b"global:create_session");
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    
    // Build instruction data
    let mut data = disc.to_vec();
    data.extend_from_slice(session_key.as_ref());
    
    // Duration (i64)
    let duration = duration_hours.unwrap_or(24);
    data.extend_from_slice(&duration.to_le_bytes());
    
    // Spending limit (u64)
    let limit = spending_limit.unwrap_or(500_000_000); // 0.5 SOL
    data.extend_from_slice(&limit.to_le_bytes());
    
    // Max wager (u64)
    let wager = max_wager.unwrap_or(10_000_000_000); // 10 SOL
    data.extend_from_slice(&wager.to_le_bytes());
    
    // Derive PlayerSession PDA
    let session_pda = Pubkey::find_program_address(
        &[b"player_session", &payer.to_bytes(), &session_key.to_bytes()],
        &PROGRAM_ID.parse().unwrap(),
    ).0;
    
    Instruction {
        program_id: PROGRAM_ID.parse().unwrap(),
        accounts: vec![
            solana_sdk::instruction::AccountMeta::new(*payer, true),
            solana_sdk::instruction::AccountMeta::new(session_pda, false),
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::ID, false),
        ],
        data,
    }
}

/// Authorizes a session key on-chain by sending the CreateSession transaction.
///
/// # Arguments
/// * `rpc_client` - Solana RPC client
/// * `payer` - The wallet keypair paying for the transaction
/// * `session_key` - The session key to authorize
/// * `duration_hours` - Session duration in hours
/// * `spending_limit` - Max lamports the session can spend
/// * `max_wager` - Max lamports per wager
///
/// # Returns
/// The transaction signature
pub async fn authorize_session_on_chain(
    rpc_client: &RpcClient,
    payer: &Keypair,
    session_key: &Pubkey,
    duration_hours: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
) -> Result<String> {
    let instruction = create_session_instruction(
        &payer.pubkey(),
        session_key,
        duration_hours,
        spending_limit,
        max_wager,
    );
    
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    Ok(signature.to_string())
}

/// Revokes a session key on-chain by sending the RevokeSession transaction.
///
/// # Arguments
/// * `rpc_client` - Solana RPC client
/// * `payer` - The wallet keypair
/// * `session_key` - The session key to revoke
///
/// # Returns
/// The transaction signature
pub async fn revoke_session_on_chain(
    rpc_client: &RpcClient,
    payer: &Keypair,
    session_key: &Pubkey,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    
    // Compute Anchor discriminator for "revoke_session"
    let mut hasher = Sha256::new();
    hasher.update(b"global:revoke_session");
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    
    // Derive PlayerSession PDA
    let session_pda = Pubkey::find_program_address(
        &[b"player_session", &payer.to_bytes(), &session_key.to_bytes()],
        &PROGRAM_ID.parse().unwrap(),
    ).0;
    
    let instruction = Instruction {
        program_id: PROGRAM_ID.parse().unwrap(),
        accounts: vec![
            solana_sdk::instruction::AccountMeta::new(payer.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(session_pda, false),
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: disc.to_vec(),
    };
    
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    Ok(signature.to_string())
}
