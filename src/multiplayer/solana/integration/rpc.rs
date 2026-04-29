#![allow(dead_code)]
use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::solana::instructions::GAME_SEED;

pub async fn initiate_game_on_chain(
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    wager_amount: u64,
) -> Result<u64, String> {
    use crate::solana::instructions::create_game_ix;
    use rand::Rng;

    let mut rng = rand::rng();
    let game_id: u64 = rng.random();

    // fee_payer = same keypair in this hot-wallet path (no separate VPS session)
    let ix = create_game_ix(
        program_id,
        keypair.pubkey(),
        keypair.pubkey(),
        game_id,
        wager_amount,
        if wager_amount > 0 { 2 } else { 0 }, // match_type: Free=0, Wager=2
        "US",
        300, // base_time_seconds: Blitz 5+0
        0,   // increment_seconds
    )
    .map_err(|e| format!("build create_game_ix: {}", e))?;

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
    use crate::solana::instructions::join_game_ix;

    // Read white player from on-chain game account
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let game_data = rpc_client
        .get_account_data(&game_pda)
        .map_err(|e| format!("Failed to fetch game account: {}", e))?;
    const WHITE_OFFSET: usize = 8 + 8;
    if game_data.len() < WHITE_OFFSET + 32 {
        return Err("game account too small".to_string());
    }
    let white_bytes: [u8; 32] = game_data[WHITE_OFFSET..WHITE_OFFSET + 32]
        .try_into()
        .map_err(|_| "bad white bytes".to_string())?;
    let white_player = Pubkey::from(white_bytes);

    // fee_payer = same keypair in this hot-wallet path
    let ix = join_game_ix(program_id, keypair.pubkey(), white_player, keypair.pubkey(), game_id)
        .map_err(|e| format!("build join_game_ix: {}", e))?;

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
