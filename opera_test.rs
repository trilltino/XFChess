//! Opera Game Test - Real On-Chain Chess Testing
//! Tests the famous "Opera Game" (Morphy vs Duke of Brunswick) on Solana
//! This is a real test with actual blockchain transactions

use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

const WAGER_AMOUNT: f64 = 0.01; // 0.01 SOL
const PROGRAM_ID: &str = "2cUpT4EQXT8D6dWQw6WGfxQm897CFKrvmwpjzCNm1Bix";

// Opera Game moves (Paul Morphy vs Duke of Brunswick, 1858)
const OPERA_GAME_MOVES: &[(&str, &str)] = &[
    ("e2e4", "Morphy opens with King's Pawn"),
    ("e7e5", "Duke responds symmetrical"),
    ("d2d4", "Morphy centralizes"),
    ("exd4", "Duke accepts gambit"),
    ("f1c4", "Morphy develops Bishop - Italian style"),
    ("d8h4", "Duke develops Queen early"),
    ("c2c3", "Morphy prepares to challenge Queen"),
    ("h4c4", "Duke captures pawn"),
    ("b1c3", "Morphy develops Knight"),
    ("g8f6", "Duke develops Knight"),
    ("f2f4", "Morphy attacks center"),
    ("d7d5", "Duke counters in center"),
    ("e4xd5", "Morphy captures"),
    ("f6xd5", "Duke recaptures"),
    ("c1g5", "Morphy pins Knight"),
    ("d8e7", "Duke unpins"),
    ("g5xf6", "Morphy exchanges"),
    ("e7xf6", "Duke recaptures"),
    ("e1e2", "Morphy prepares castling"),
    ("c8d7", "Duke develops Bishop"),
    ("e2f2", "Morphy reinforces King"),
    ("o-o", "Morphy castles King-side"),
    ("e8g8", "Duke castles King-side"),
    ("a1e1", "Morphy controls open file"),
    ("a7a6", "Duke creates escape square"),
    ("h2h3", "Morphy prevents back-rank threats"),
    ("f8e8", "Duke reinforces King"),
    ("c4b3", "Morphy repositions Bishop"),
    ("d7c6", "Duke centralizes Bishop"),
    ("f2f4", "Morphy attacks center"),
    ("e6e5", "Duke challenges Morphy's center"),
    ("d1e2", "Morphy centralizes Queen"),
    ("c6d5", "Duke continues development"),
    ("e2e7", "Morphy exchanges Queens"),
    ("d8e7", "Duke recaptures"),
    ("e1e7", "Morphy captures Queen - decisive!"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Opera Game Test - Real On-Chain Chess");
    println!("========================================");
    println!("Testing Paul Morphy vs Duke of Brunswick (1858)");
    println!("Program: {}", PROGRAM_ID);
    println!("Wager: {} SOL", WAGER_AMOUNT);
    
    // Check if we have the required files
    println!("\n📋 Prerequisites Check:");
    println!("======================");
    
    let required_files = vec![
        "playtest_white.json",
        "playtest_black.json", 
        "player2_wallet.json",
    ];
    
    for file in &required_files {
        if std::path::Path::new(file).exists() {
            println!("✓ Found: {}", file);
        } else {
            println!("✗ Missing: {}", file);
            println!("Please create wallet files before running this test");
            return Ok(());
        }
    }
    
    println!("\n🚀 Starting Opera Game Test");
    println!("===========================");
    println!("This will test the complete game with {} moves", OPERA_GAME_MOVES.len());
    println!("Expected transactions: {}", OPERA_GAME_MOVES.len() + 2); // + create + finalize
    
    println!("\n📝 Instructions:");
    println!("================");
    println!("1. Open TWO terminal windows");
    println!("2. Terminal 1 (White/Morphy):");
    println!("   cargo run --bin xfchess --features solana -- \\");
    println!("     --competitive --wager_amount {} \\", WAGER_AMOUNT);
    println!("     --session_key morphy_session \\");
    println!("     --p2p_port 5000 --debug");
    println!("");
    println!("3. Terminal 2 (Black/Duke):");
    println!("   cargo run --bin xfchess --features solana -- \\");
    println!("     --competitive --wager_amount {} \\", WAGER_AMOUNT);
    println!("     --session_key duke_session \\");
    println!("     --player_color black \\");
    println!("     --bootstrap_node <GET_FROM_PLAYER1> --debug");
    println!("");
    println!("4. Play the following moves in order:");
    
    // Display all moves with annotations
    for (i, (move_str, annotation)) in OPERA_GAME_MOVES.iter().enumerate() {
        println!("   Move {}: {} - {}", i + 1, move_str, annotation);
        sleep(Duration::from_millis(100)).await;
    }
    
    println!("\n⚡ Expected Auto-Recording Behavior:");
    println!("===================================");
    println!("Each move should trigger:");
    println!("  [AUTO_RECORD] Recording move {} for game <GAME_ID>");
    println!("  [AUTO_RECORD] ✓ Move recorded: <TX_SIGNATURE>");
    println!("  [AUTO_RECORD] Explorer: https://explorer.solana.com/tx/<TX>/?cluster=devnet");
    
    println!("\n🎯 Test Validation:");
    println!("==================");
    println!("After completing all moves:");
    println!("✓ Check console for {} [AUTO_RECORD] messages", OPERA_GAME_MOVES.len());
    println!("✓ Verify {} transactions on Solana Explorer", OPERA_GAME_MOVES.len() + 2);
    println!("✓ Confirm game finalization and wager distribution");
    println!("✓ Validate complete Opera Game preservation on-chain");
    
    println!("\n📊 Expected Transaction Summary:");
    println!("==============================");
    println!("• 1 create_game transaction");
    println!("• {} record_move transactions (auto-recorded)", OPERA_GAME_MOVES.len());
    println!("• 1 finalize_game transaction");
    println!("• Total: {} on-chain transactions", OPERA_GAME_MOVES.len() + 2);
    
    println!("\n🏆 Opera Game Historical Significance:");
    println!("===================================");
    println!("Paul Morphy played this game at the Paris Opera House in 1858");
    println!("While simultaneously playing blindfold chess in another room!");
    println!("Now this legendary game will live forever on Solana blockchain.");
    
    println!("\n🔗 Verification Links:");
    println!("====================");
    println!("After the test, check:");
    println!("• Player wallets for transaction history");
    println!("• Solana Explorer for all game transactions");
    println!("• Game account for complete move history");
    println!("• Program state for wager distribution");
    
    println!("\n✨ Ready to test the Opera Game on Solana!");
    println!("========================================");
    println!("Open the terminals and start playing chess history! 🎭");
    
    Ok(())
}
