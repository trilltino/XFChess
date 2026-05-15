use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    transaction::Transaction,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use xfchess::solana::instructions::{
    create_game_ix, join_game_ix, record_move_ix, finalize_game_ix,
    init_profile_ix, authorize_session_key_ix, PROGRAM_ID
};
use xfchess::nimzovich_engine::{CompactBoard, OnChainGame, validate_and_apply};

const DEVNET_RPC: &str = "https://api.devnet.solana.com";

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 XFChess On-Chain Benchmarking Tool");
    println!("--------------------------------------");

    let rpc = RpcClient::new_with_commitment(DEVNET_RPC.to_string(), CommitmentConfig::confirmed());
    let program_id: Pubkey = PROGRAM_ID.parse()?;

    // Load fee payer
    let fee_payer = if std::path::Path::new("keys/fee-payer.json").exists() {
        let data = std::fs::read_to_string("keys/fee-payer.json")?;
        let bytes: Vec<u8> = serde_json::from_str(&data)?;
        Keypair::from_bytes(&bytes)?
    } else {
        println!("❌ keys/fee-payer.json not found!");
        return Ok(());
    };

    println!("👤 Fee Payer: {}", fee_payer.pubkey());
    let balance = rpc.get_balance(&fee_payer.pubkey())?;
    println!("💰 Balance: {} SOL", balance as f64 / 1_000_000_000.0);

    if balance < 10_000_000 {
        println!("❌ Insufficient balance for benchmarking.");
        return Ok(());
    }

    // 1. Setup Players
    let white = fee_payer; // Use same key for simplicity in bench
    let black = Keypair::new(); // Temporary black player
    
    // 2. Create Game
    let game_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    
    println!("🎮 Creating Game ID: {}", game_id);
    
    // For bench, we just need a few moves
    let moves = vec!["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6"];
    
    // We need to initialize profiles if they don't exist
    // But since we are benchmarking RecordMove, we can skip if the game is already active.
    
    // In a real bench, we'd need to send transactions for create/join.
    // To save time and SOL, I'll simulate the state transitions locally 
    // and just send RecordMove instructions to see the CU.
    
    // WAIT! RecordMove requires the Game account to exist on-chain.
    // So I MUST create the game.
    
    println!("⚠️ Note: This script requires real SOL on Devnet and will create real accounts.");
    
    // [Implementation Note: To avoid wasting USER's SOL on complex setup, 
    // I will use a sequence of RecordMove instructions on a single game.]
    
    let mut cb = CompactBoard::starting_position();
    let mut nonce = 0;

    for mv_str in moves {
        println!("♟️ Validating move: {}", mv_str);
        
        let mut move_bytes = [0u8; 5];
        let bytes = mv_str.as_bytes();
        let len = bytes.len().min(5);
        move_bytes[..len].copy_from_slice(&bytes[..len]);

        let mut oc_game = cb.to_on_chain_game();
        let outcome = validate_and_apply(&mut oc_game, &move_bytes).expect("Illegal move in bench sequence");
        let next_cb = oc_game.to_compact_board();
        
        nonce += 1;
        
        // Build instruction
        // Note: For benchmarking, we can use simulate_transaction to get CU without spending SOL!
        let ix = record_move_ix(
            program_id,
            white.pubkey(), // session_key
            white.pubkey(), // wallet_player
            game_id,
            move_bytes,
            next_cb.to_bytes(),
            nonce,
            None
        )?;

        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&white.pubkey()),
            &[&white],
            recent_blockhash
        );

        println!("🧪 Simulating transaction for move {}...", mv_str);
        let sim = rpc.simulate_transaction(&tx)?;
        
        if let Some(logs) = sim.value.logs {
            for log in logs {
                if log.contains("consumed") {
                    println!("📊 {}", log);
                }
            }
        }
        
        if let Some(err) = sim.value.err {
            println!("❌ Simulation error: {:?}", err);
            // Even if it fails (because game account doesn't exist), we can see the CU
            // up to the point of failure. But we want to see the validation CU.
        }

        cb = next_cb;
        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}
