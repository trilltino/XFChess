use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

use crate::solana::instructions::{GAME_SEED, MOVE_LOG_SEED, WAGER_ESCROW_SEED};

pub async fn initiate_game_on_chain(
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    wager_amount: u64,
) -> Result<u64, String> {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let game_id: u64 = rng.random();

    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let move_log_pda =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = vec![0]; // discriminator
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(0); // GameType::PvP

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[&keypair],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("On-chain game created: {}", game_id);
    Ok(game_id)
}

pub async fn join_game_on_chain(
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    game_id: u64,
) -> Result<u64, String> {
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let escrow_pda =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id).0;

    let mut data = vec![1]; // discriminator for join_game
    data.extend_from_slice(&game_id.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|e| format!("Failed to get blockhash: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[&keypair],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| format!("Failed to send transaction: {}", e))?;

    info!("Successfully joined on-chain game: {}", game_id);
    Ok(game_id)
}

pub fn prepare_final_game_state(
    moves: Vec<String>,
    winner: Option<String>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let game_result = GameResult {
        moves,
        winner,
        timestamp: chrono::Utc::now().timestamp(),
    };

    let serialized = bincode::serialize(&game_result)?;
    Ok(serialized)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GameResult {
    moves: Vec<String>,
    winner: Option<String>,
    timestamp: i64,
}
