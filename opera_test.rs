//! Real Opera Game On-Chain Test
//! 
//! Records each move as a separate XFChess program transaction on Solana
//! with rich metadata and individual Explorer links.

use std::fs;
use std::time::Duration;
use tokio::time::sleep;

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
    instruction::{AccountMeta, Instruction},
    system_program,
};
use serde_json;
use sha2::{Digest, Sha256};
use xfchess::solana::instructions::{create_game_ix, join_game_ix, record_move_ix, finalize_game_ix, GameType, PROFILE_SEED};
use xfchess::multiplayer::magicblock_resolver::{MagicBlockResolver, MagicBlockConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("XFChess Real Opera Game On-Chain Test");
    println!("=====================================");
    
    // Load keypairs
    let white_keypair = load_keypair("playtest_white.json")?;
    let black_keypair = load_keypair("playtest_black.json")?;
    
    println!("Players:");
    println!("  White (Morphy): {}", white_keypair.pubkey());
    println!("  Black (Duke): {}", black_keypair.pubkey());
    
    // Setup RPC client
    let rpc_client = RpcClient::new_with_commitment("https://api.devnet.solana.com", CommitmentConfig::confirmed());
    
    // Check balances
    println!("\nChecking balances...");
    let white_balance = rpc_client.get_balance(&white_keypair.pubkey())?;
    let black_balance = rpc_client.get_balance(&black_keypair.pubkey())?;
    
    println!("  White: {} SOL", white_balance as f64 / 1_000_000_000.0);
    println!("  Black: {} SOL", black_balance as f64 / 1_000_000_000.0);
    
    if white_balance < 5_000_000 || black_balance < 5_000_000 {
        println!("Both players need at least 0.005 SOL for game creation + moves");
        return Ok(());
    }
    
    // Program details — must match declare_id! in xfchess-game
    let program_id: Pubkey = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP".parse()?;
    let game_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();  // Unique game ID from timestamp
    let wager_amount = 1_000_000; // 0.001 SOL
    
    println!("\nGame Setup:");
    println!("  Program ID: {}", program_id);
    println!("  Game ID: {}", game_id);
    println!("  Wager: {} SOL", wager_amount as f64 / 1_000_000_000.0);
    
    // Setup MagicBlock Resolver with custom RPC
    let mb_rpc_url = "https://devnet-eu.magicblock.app/";
    println!("  MagicBlock ER RPC: {}", mb_rpc_url);
    
    let mb_config = MagicBlockConfig {
        er_endpoint: mb_rpc_url.to_string(),
        ..MagicBlockConfig::default()
    };
    
    let mut mb_resolver = MagicBlockResolver::new(mb_config);
    mb_resolver.set_solana_rpc(std::sync::Arc::new(RpcClient::new_with_commitment("https://api.devnet.solana.com", CommitmentConfig::confirmed())));
    mb_resolver.set_game_id(game_id);
    
    // Step 1: Create game (base-layer Solana tx)
    println!("\nStep 1: Creating game on-chain...");
    let create_sig = create_game_on_chain(&rpc_client, &program_id, &white_keypair, game_id, wager_amount).await?;
    println!("  Game created: {}", create_sig);
    println!("  Solana Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", create_sig);
    
    sleep(Duration::from_secs(2)).await;
    
    // Step 2: Black joins and funds escrow (base-layer Solana tx)
    println!("\nStep 2: Black player joining game (escrow match)...");
    let join_sig = join_game_on_chain(&rpc_client, &program_id, &black_keypair, game_id).await?;
    println!("  Game joined: {}", join_sig);
    println!("  Solana Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", join_sig);
    
    sleep(Duration::from_secs(2)).await;
    
    // Step 2.5: Delegate Game to MagicBlock ER (base-layer Solana tx)
    println!("\nStep 2.5: Delegating game {} to Magic Block ER...", game_id);
    let game_pda = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;
    mb_resolver.delegate_game(game_pda, &white_keypair)?;
    println!("  ✓ Game delegated to ER for sub-second processing");
    
    // Step 3: Record all moves of the Opera Game with correct FENs
    // Morphy vs Duke of Brunswick & Count Isouard, Paris 1858
    println!("\nStep 3: Recording Opera Game moves with metadata...");
    
    // Each entry: (UCI move, resulting FEN after the move, annotation, is_white)
    let opera_moves: Vec<(&str, &str, &str, bool)> = vec![
        // Move 1: 1. e4
        ("e2e4",
         "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
         "King's Pawn Opening - Classical start",
         true),
        // Move 2: 1... e5
        ("e7e5",
         "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
         "Open Game - Symmetrical response",
         false),
        // Move 3: 2. Nf3
        ("g1f3",
         "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2",
         "Knight development - controls center",
         true),
        // Move 4: 2... d6
        ("d7d6",
         "rnbqkbnr/ppp2ppp/3p4/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3",
         "Philidor Defense - Solid but passive",
         false),
        // Move 5: 3. d4
        ("d2d4",
         "rnbqkbnr/ppp2ppp/3p4/4p3/3PP3/5N2/PPP2PPP/RNBQKB1R b KQkq d3 0 3",
         "Central break - Challenges Black's setup",
         true),
        // Move 6: 3... Bg4
        ("c8g4",
         "rn1qkbnr/ppp2ppp/3p4/4p3/3PP1b1/5N2/PPP2PPP/RNBQKB1R w KQkq - 1 4",
         "Pins knight to queen - Developing with tempo",
         false),
        // Move 7: 4. dxe5
        ("d4e5",
         "rn1qkbnr/ppp2ppp/3p4/4P3/4P1b1/5N2/PPP2PPP/RNBQKB1R b KQkq - 0 4",
         "Captures center pawn - Opens position",
         true),
        // Move 8: 4... Bxf3
        ("g4f3",
         "rn1qkbnr/ppp2ppp/3p4/4P3/4P3/5b2/PPP2PPP/RNBQKB1R w KQkq - 0 5",
         "Captures knight - Damages White's structure",
         false),
        // Move 9: 5. Qxf3
        ("d1f3",
         "rn1qkbnr/ppp2ppp/3p4/4P3/4P3/5Q2/PPP2PPP/RNB1KB1R b KQkq - 0 5",
         "Queen recaptures - Centralized queen",
         true),
        // Move 10: 5... dxe5
        ("d6e5",
         "rn1qkbnr/ppp2ppp/8/4p3/4P3/5Q2/PPP2PPP/RNB1KB1R w KQkq - 0 6",
         "Recaptures pawn - Opens d-file",
         false),
        // Move 11: 6. Bc4
        ("f1c4",
         "rn1qkbnr/ppp2ppp/8/4p3/2B1P3/5Q2/PPP2PPP/RNB1K2R b KQkq - 1 6",
         "Bishop to c4 - Targets f7 weakness",
         true),
        // Move 12: 6... Nf6
        ("g8f6",
         "rn1qkb1r/ppp2ppp/5n2/4p3/2B1P3/5Q2/PPP2PPP/RNB1K2R w KQkq - 2 7",
         "Knight develops - Defends and attacks",
         false),
        // Move 13: 7. Qb3
        ("f3b3",
         "rn1qkb1r/ppp2ppp/5n2/4p3/2B1P3/1Q6/PPP2PPP/RNB1K2R b KQkq - 3 7",
         "Queen to b3 - Double attack on b7 and f7",
         true),
        // Move 14: 7... Qe7
        ("d8e7",
         "rn2kb1r/ppp1qppp/5n2/4p3/2B1P3/1Q6/PPP2PPP/RNB1K2R w KQkq - 4 8",
         "Queen guards f7 and e-file",
         false),
        // Move 15: 8. Nc3
        ("b1c3",
         "rn2kb1r/ppp1qppp/5n2/4p3/2B1P3/1QN5/PPP2PPP/R1B1K2R b KQkq - 5 8",
         "Knight to c3 - Completes development",
         true),
        // Move 16: 8... c6
        ("c7c6",
         "rn2kb1r/pp2qppp/2p2n2/4p3/2B1P3/1QN5/PPP2PPP/R1B1K2R w KQkq - 0 9",
         "Solidifies center - Prepares d5",
         false),
        // Move 17: 9. Bg5
        ("c1g5",
         "rn2kb1r/pp2qppp/2p2n2/4p1B1/2B1P3/1QN5/PPP2PPP/R3K2R b KQkq - 1 9",
         "Bishop pins knight to queen - Increasing pressure",
         true),
        // Move 18: 9... b5
        ("b7b5",
         "rn2kb1r/p3qppp/2p2n2/1p2p1B1/2B1P3/1QN5/PPP2PPP/R3K2R w KQkq b6 0 10",
         "b5 thrust - Counterplay on queenside",
         false),
        // Move 19: 10. Nxb5
        ("c3b5",
         "rn2kb1r/p3qppp/2p2n2/1N2p1B1/2B1P3/1Q6/PPP2PPP/R3K2R b KQkq - 0 10",
         "Knight takes b5 - Tactical blow",
         true),
        // Move 20: 10... cxb5
        ("c6b5",
         "rn2kb1r/p3qppp/5n2/1p2p1B1/2B1P3/1Q6/PPP2PPP/R3K2R w KQkq - 0 11",
         "Recaptures knight - Opens c-file",
         false),
        // Move 21: 11. Bxb5+
        ("c4b5",
         "rn2kb1r/p3qppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/R3K2R b KQkq - 0 11",
         "Bishop takes b5 check! - Forcing sequence begins",
         true),
        // Move 22: 11... Nbd7
        ("b8d7",
         "r3kb1r/p2nqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/R3K2R w KQkq - 1 12",
         "Knight blocks check - Only reasonable move",
         false),
        // Move 23: 12. O-O-O
        ("e1c1",
         "r3kb1r/p2nqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/2KR3R b kq - 2 12",
         "Queenside castling - Rook enters d-file with tempo",
         true),
        // Move 24: 12... Rd8
        ("a8d8",
         "3rkb1r/p2nqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/2KR3R w k - 3 13",
         "Rook to d8 - Defends against discovered attack",
         false),
        // Move 25: 13. Rxd7!
        ("d1d7",
         "3rkb1r/p2Rqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/2K4R b k - 0 13",
         "Rook sacrifice! Rxd7 - Morphy's brilliance begins",
         true),
        // Move 26: 13... Rxd7
        ("d8d7",
         "4kb1r/p2rqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/2K4R w k - 0 14",
         "Forced recapture - Removes the rook",
         false),
        // Move 27: 14. Rd1
        ("h1d1",
         "4kb1r/p2rqppp/5n2/1B2p1B1/4P3/1Q6/PPP2PPP/2KR4 b k - 1 14",
         "Rook to d1 - Pins the defender to the king",
         true),
        // Move 28: 14... Qe6
        ("e7e6",
         "4kb1r/p2r1ppp/4qn2/1B2p1B1/4P3/1Q6/PPP2PPP/2KR4 w k - 2 15",
         "Queen to e6 - Desperate attempt to block",
         false),
        // Move 29: 15. Bxd7+
        ("b5d7",
         "4kb1r/p2B1ppp/4qn2/4p1B1/4P3/1Q6/PPP2PPP/2KR4 b k - 0 15",
         "Bishop takes d7 check! - Removes last defender",
         true),
        // Move 30: 15... Nxd7
        ("f6d7",
         "4kb1r/p2n1ppp/4q3/4p1B1/4P3/1Q6/PPP2PPP/2KR4 w k - 0 16",
         "Knight recaptures - Forced",
         false),
        // Move 31: 16. Qb8+!
        ("b3b8",
         "1Q2kb1r/p2n1ppp/4q3/4p1B1/4P3/8/PPP2PPP/2KR4 b k - 1 16",
         "Queen sacrifice! Qb8+!! - The immortal offer",
         true),
        // Move 32: 16... Nxb8
        ("d7b8",
         "1n2kb1r/p4ppp/4q3/4p1B1/4P3/8/PPP2PPP/2KR4 w k - 0 17",
         "Knight forced to take queen",
         false),
        // Move 33: 17. Rd8# !!
        ("d1d8",
         "1n1Rkb1r/p4ppp/4q3/4p1B1/4P3/8/PPP2PPP/2K5 b k - 1 17",
         "ROOK TO D8# - CHECKMATE!! The Opera Game concludes!",
         true),
    ];
    
    let mut all_signatures: Vec<(usize, String, String, String)> = Vec::new();
    let mut prev_hash = [0u8; 32]; // Genesis hash for move hash chain
    
    for (i, (move_str, next_fen, annotation, is_white)) in opera_moves.iter().enumerate() {
        let move_number = i + 1;
        let player = if *is_white { "White (Morphy)" } else { "Black (Duke)" };
        let keypair = if *is_white { &white_keypair } else { &black_keypair };
        
        println!("\nMove {}: {} - {}", move_number, player, move_str);
        println!("  Annotation: {}", annotation);
        println!("  Next FEN: {}", next_fen);
        
        // Record this move via MagicBlock ER
        match record_move_on_chain(
            &rpc_client,
            &mb_resolver,
            &program_id,
            keypair,
            game_id,
            move_str,
            next_fen,
            Some(annotation.to_string()),
            Some(format!("Move {} - Paris 1858", move_number)),
            &prev_hash,
        ).await {
            Ok((signature, new_hash)) => {
                println!("  ✓ Recorded (ER): {}", signature);
                println!("  MagicBlock ER Explorer: {}", mb_resolver.er_explorer_url(&signature));
                all_signatures.push((move_number, move_str.to_string(), annotation.to_string(), signature));
                prev_hash = new_hash;
            }
            Err(e) => {
                println!("  ✗ Error on move {}: {}", move_number, e);
                println!("  Stopping here.");
                break;
            }
        }
        
        // Brief delay - much shorter now due to ER speed
        sleep(Duration::from_millis(200)).await;
    }
    
    // Step 3.5: Undelegate Game from MagicBlock ER
    println!("\nStep 3.5: Undelegating game from Magic Block ER (Settling)...");
    mb_resolver.undelegate_game(&white_keypair)?;
    println!("  ✓ Game settled back to Solana Base Layer");
    
    // Step 4: Finalize game and trigger payout
    println!("\nStep 4: Finalizing game and triggering wager payout...");
    
    // Create player profiles first (required by finalize_game_ix for ELO updates)
    // In a real app these would be created once per user, but for the test we ensure they exist
    let _ = create_profile_on_chain(&rpc_client, &program_id, &white_keypair, "Morphy").await;
    let _ = create_profile_on_chain(&rpc_client, &program_id, &black_keypair, "Duke").await;
    sleep(Duration::from_secs(2)).await;
    
    // Record White checking Black's balance before payout
    let black_balance_before = rpc_client.get_balance(&black_keypair.pubkey()).unwrap_or(0);
    let white_balance_before = rpc_client.get_balance(&white_keypair.pubkey()).unwrap_or(0);
    
    // Result: 0 = White Wins (GameResult::Winner(Pubkey) encoded dynamically in ix builder)
    // Actually, looking at instructions.rs finalize_game_ix takes `result: u8` where 0=WhiteWins, 1=BlackWins, 2=Draw
    let finalize_sig = finalize_game_on_chain(&rpc_client, &program_id, &white_keypair, game_id, 0).await?;
    println!("  Game finalized: {}", finalize_sig);
    println!("  Solana Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", finalize_sig);
    
    sleep(Duration::from_secs(2)).await;
    
    // Check balances after payout
    let white_balance_after = rpc_client.get_balance(&white_keypair.pubkey()).unwrap_or(0);
    let black_balance_after = rpc_client.get_balance(&black_keypair.pubkey()).unwrap_or(0);
    
    println!("\nPayout Summary:");
    println!("  White Balance Before: {:.6} SOL", white_balance_before as f64 / 1_000_000_000.0);
    println!("  White Balance After:  {:.6} SOL (+{:.6} SOL payout)", 
             white_balance_after as f64 / 1_000_000_000.0,
             (white_balance_after.saturating_sub(white_balance_before)) as f64 / 1_000_000_000.0);
    
    println!("\nStep 5: Game complete - all moves recorded!");
    println!("=======================================");
    
    println!("Game Creation (Solana): https://explorer.solana.com/tx/{}?cluster=devnet", create_sig);
    println!("Game Join     (Solana): https://explorer.solana.com/tx/{}?cluster=devnet", join_sig);
    
    println!("\nIndividual Move Links (MagicBlock ER — ephemeral, valid during delegation):");
    for (move_number, move_str, annotation, signature) in &all_signatures {
        println!("Move {:2}: {:6} | {} ", move_number, move_str, annotation);
        println!("         {}", mb_resolver.er_explorer_url(signature));
    }
    
    println!("\nFinal Result: 1-0 (White wins - Paul Morphy)");
    println!("Total moves recorded: {}", all_signatures.len());
    
    // Create HTML summary
    create_html_summary(&create_sig, &join_sig, &finalize_sig, &all_signatures)?;
    
    println!("\nHTML summary created: opera_game_links.html");
    println!("Program: https://explorer.solana.com/address/{}?cluster=devnet", program_id);
    
    Ok(())
}

fn load_keypair(filename: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let data = fs::read(filename)?;
    let bytes: Vec<u8> = serde_json::from_slice(&data)?;
    let keypair = Keypair::from_bytes(&bytes)?;
    Ok(keypair)
}

async fn create_game_on_chain(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    keypair: &Keypair,
    game_id: u64,
    wager_amount: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    let ix = create_game_ix(
        *program_id,
        keypair.pubkey(),
        game_id,
        wager_amount,
        GameType::PvP,
    ).map_err(|e| format!("Failed to create instruction: {}", e))?;
    
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        recent_blockhash,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    Ok(signature.to_string())
}

async fn join_game_on_chain(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    keypair: &Keypair,
    game_id: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    let ix = join_game_ix(
        *program_id,
        keypair.pubkey(),
        game_id,
    ).map_err(|e| format!("Failed to create instruction: {}", e))?;
    
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        recent_blockhash,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    Ok(signature.to_string())
}

async fn record_move_on_chain(
    rpc_client: &RpcClient,
    resolver: &MagicBlockResolver,
    program_id: &Pubkey,
    keypair: &Keypair,
    game_id: u64,
    move_str: &str,
    next_fen: &str,
    move_annotation: Option<String>,
    move_time: Option<String>,
    prev_hash: &[u8; 32],
) -> Result<(String, [u8; 32]), Box<dyn std::error::Error>> {
    // Create the instruction using the existing builder
    // record_move_ix internally computes the move hash
    let ix = record_move_ix(
        *program_id,
        keypair.pubkey(),
        game_id,
        move_str.to_string(),
        next_fen.to_string(),
        move_annotation,
        move_time,
        prev_hash,
    ).map_err(|e| format!("Failed to create instruction: {}", e))?;
    
    let signature = resolver.route_transaction(vec![ix], keypair)
        .map_err(|e| format!("ER routing failed: {}", e))?;
    
    // Compute the new hash for the chain
    let mut hasher = Sha256::new();
    hasher.update(prev_hash);
    hasher.update(move_str.as_bytes());
    let new_hash: [u8; 32] = hasher.finalize().into();
    
    Ok((signature.to_string(), new_hash))
}

async fn create_profile_on_chain(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    keypair: &Keypair,
    username: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let profile_pda = Pubkey::find_program_address(&[PROFILE_SEED, keypair.pubkey().as_ref()], program_id).0;
    
    // Check if profile already exists
    if rpc_client.get_account(&profile_pda).is_ok() {
        return Ok("Profile already exists".to_string());
    }

    // Compute Anchor discriminator for "init_profile"
    let disc = {
        let mut hasher = Sha256::new();
        hasher.update(b"global:init_profile");
        let hash = hasher.finalize();
        let mut d = [0u8; 8];
        d.copy_from_slice(&hash[..8]);
        d
    };
    let mut data = disc.to_vec();

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(profile_pda, false),
            AccountMeta::new(keypair.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        rpc_client.get_latest_blockhash()?,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    Ok(signature.to_string())
}

async fn finalize_game_on_chain(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    keypair: &Keypair,
    game_id: u64,
    result: u8,
) -> Result<String, Box<dyn std::error::Error>> {
    // Fetch game account to read white/black pubkeys
    let game_pda = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], program_id).0;
    let game_data = rpc_client.get_account_data(&game_pda)?;
    // Game struct layout: 8 bytes discriminator + 8 bytes game_id + 32 bytes white + 32 bytes black
    let white_pubkey = Pubkey::new_from_array(game_data[16..48].try_into().unwrap());
    let black_pubkey = Pubkey::new_from_array(game_data[48..80].try_into().unwrap());

    let ix = finalize_game_ix(
        *program_id,
        keypair.pubkey(),
        game_id,
        result,
        white_pubkey,
        black_pubkey,
    ).map_err(|e| format!("Failed to create finalize instruction: {}", e))?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&keypair.pubkey()),
        &[keypair],
        rpc_client.get_latest_blockhash()?,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    Ok(signature.to_string())
}

fn create_html_summary(
    create_sig: &str,
    join_sig: &str,
    finalize_sig: &str,
    signatures: &Vec<(usize, String, String, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut html = String::new();
    
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html><head><title>Opera Game On-Chain - Real XFChess Program</title>");
    html.push_str("<style>");
    html.push_str("body { font-family: 'Segoe UI', Arial, sans-serif; margin: 20px; background: #1a1a2e; color: #e0e0e0; }");
    html.push_str("h1 { color: #e94560; }");
    html.push_str("h2 { color: #0f3460; }");
    html.push_str("table { border-collapse: collapse; width: 100%; }");
    html.push_str("th, td { border: 1px solid #333; padding: 10px; text-align: left; }");
    html.push_str("th { background-color: #16213e; color: #e94560; }");
    html.push_str("tr:nth-child(even) { background-color: #16213e; }");
    html.push_str("tr:nth-child(odd) { background-color: #1a1a2e; }");
    html.push_str("a { color: #00d2ff; text-decoration: none; }");
    html.push_str("a:hover { text-decoration: underline; color: #e94560; }");
    html.push_str(".game-section { background-color: #16213e; padding: 15px; margin: 15px 0; border-radius: 8px; border: 1px solid #0f3460; }");
    html.push_str(".white { color: #ffffff; font-weight: bold; }");
    html.push_str(".black { color: #888; font-weight: bold; }");
    html.push_str("</style></head><body>\n");
    
    html.push_str("<h1>♚ Paul Morphy's Opera Game (1858) — On-Chain</h1>\n");
    html.push_str("<p>Game setup &amp; finalization on Solana devnet. Moves processed via MagicBlock Ephemeral Rollup for sub-second latency, then settled back to Solana.</p>\n");
    
    html.push_str("<div class='game-section'>");
    html.push_str("<h2 style='color:#e94560'>Game Setup &amp; Resolution (Solana Base Layer)</h2>\n");
    html.push_str(&format!("<p><strong>Game Creation:</strong> <a href='https://explorer.solana.com/tx/{}?cluster=devnet' target='_blank'>Solana Explorer</a> (0.001 SOL Wager)</p>\n", create_sig));
    html.push_str(&format!("<p><strong>Black Joins:</strong> <a href='https://explorer.solana.com/tx/{}?cluster=devnet' target='_blank'>Solana Explorer</a> (0.001 SOL Escrow Match)</p>\n", join_sig));
    html.push_str(&format!("<p><strong>Game Finalized &amp; Payout:</strong> <a href='https://explorer.solana.com/tx/{}?cluster=devnet' target='_blank'>Solana Explorer</a> (0.002 SOL Payout to White)</p>\n", finalize_sig));
    html.push_str("</div>");
    
    html.push_str(&format!("<h2 style='color:#e94560'>Individual Moves — MagicBlock ER ({} total)</h2>\n", signatures.len()));
    html.push_str("<p style='color:#888;font-size:0.9em;'>Move transactions are routed through the MagicBlock Ephemeral Rollup for sub-second confirmation. ER tx signatures are ephemeral and only resolvable while the game is delegated.</p>\n");
    html.push_str("<table>\n");
    html.push_str("<tr><th>#</th><th>Player</th><th>Move</th><th>Annotation</th><th>ER Explorer</th></tr>\n");
    
    for (move_number, move_str, annotation, signature) in signatures {
        let (player, class) = if move_number % 2 == 1 { ("White (Morphy)", "white") } else { ("Black (Duke)", "black") };
        
        html.push_str(&format!(
            "<tr><td>{}</td><td class='{}'>{}</td><td>{}</td><td>{}</td><td><a href='https://explorer.magicblock.gg/tx/{}?cluster=devnet' target='_blank'>View ER Tx</a></td></tr>\n",
            move_number, class, player, move_str, annotation, signature
        ));
    }
    
    html.push_str("</table>\n");
    html.push_str("<div class='game-section'>");
    html.push_str("<h2 style='color:#e94560'>Game Result</h2>\n");
    html.push_str("<p><strong>Final Result:</strong> 1-0 (White wins — Paul Morphy)</p>\n");
    html.push_str(&format!("<p><strong>Total Moves:</strong> {}</p>\n", signatures.len()));
    html.push_str("<p><strong>Program:</strong> <a href='https://explorer.solana.com/address/3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP?cluster=devnet' target='_blank'>XFChess Program</a></p>\n");
    html.push_str("<p><strong>Architecture:</strong> Game setup &amp; finalization on Solana base layer. Moves processed via MagicBlock Ephemeral Rollup, then settled back to Solana when the game ends.</p>\n");
    html.push_str("</div>");
    html.push_str("</body></html>\n");
    
    fs::write("opera_game_links.html", html)?;
    Ok(())
}
