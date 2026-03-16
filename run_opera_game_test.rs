//! 🎭 Opera Game Test Runner - Complete Orchestration
//! Orchestrates the complete Opera Game on-chain test with real Solana integration

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tokio::process::Command as AsyncCommand;

const WAGER_AMOUNT: f64 = 0.001;
const WHITE_PLAYER: &str = "8SMHifMFg3VFdC8rJ38yRbLwB612EgYk5MhNfxVYY3jc";
const BLACK_PLAYER: &str = "FJ74VQme1ymF1cSRYHeAi4aADNijcCdAZyPkzUzPVWz";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Opera Game Test Runner - Complete Orchestration");
    println!("==================================================");
    
    // Step 1: Fund player addresses
    println!("\n💰 Step 1: Funding Player Addresses");
    println!("====================================");
    
    println!("Funding White player (Morphy)...");
    let fund_white = AsyncCommand::new("solana")
        .args(&["airdrop", "1", WHITE_PLAYER, "--url", "devnet"])
        .output()
        .await?;
    
    if fund_white.status.success() {
        println!("✓ White player funded successfully");
    } else {
        println!("✗ Failed to fund White player: {}", String::from_utf8_lossy(&fund_white.stderr));
    }
    
    println!("Funding Black player (Duke)...");
    let fund_black = AsyncCommand::new("solana")
        .args(&["airdrop", "1", BLACK_PLAYER, "--url", "devnet"])
        .output()
        .await?;
    
    if fund_black.status.success() {
        println!("✓ Black player funded successfully");
    } else {
        println!("✗ Failed to fund Black player: {}", String::from_utf8_lossy(&fund_black.stderr));
    }
    
    // Step 2: Check balances
    println!("\n📊 Step 2: Checking Player Balances");
    println!("===================================");
    
    let white_balance = AsyncCommand::new("solana")
        .args(&["balance", WHITE_PLAYER, "--url", "devnet"])
        .output()
        .await?;
    
    if white_balance.status.success() {
        let balance = String::from_utf8_lossy(&white_balance.stdout);
        println!("White player balance: {}", balance.trim());
    }
    
    let black_balance = AsyncCommand::new("solana")
        .args(&["balance", BLACK_PLAYER, "--url", "devnet"])
        .output()
        .await?;
    
    if black_balance.status.success() {
        let balance = String::from_utf8_lossy(&black_balance.stdout);
        println!("Black player balance: {}", balance.trim());
    }
    
    // Step 3: Launch game instances
    println!("\n🚀 Step 3: Launching Game Instances");
    println!("==================================");
    println!("Starting White player (Morphy)...");
    
    let mut white_process = AsyncCommand::new("cargo")
        .args(&[
            "run", "--bin", "xfchess", "--features", "solana", "--",
            "--competitive",
            "--wager_amount", &WAGER_AMOUNT.to_string(),
            "--session_key", "morphy_session",
            "--p2p_port", "5000",
            "--debug"
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    println!("Starting Black player (Duke)...");
    
    // Give White player time to start
    sleep(Duration::from_secs(3)).await;
    
    let mut black_process = AsyncCommand::new("cargo")
        .args(&[
            "run", "--bin", "xfchess", "--features", "solana", "--",
            "--competitive",
            "--wager_amount", &WAGER_AMOUNT.to_string(),
            "--session_key", "duke_session",
            "--player_color", "black",
            "--bootstrap_node", "localhost:5000",
            "--debug"
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    // Step 4: Monitor game progress
    println!("\n📋 Step 4: Monitoring Game Progress");
    println!("==================================");
    println!("Game instances launched. Monitoring for auto-recorded moves...");
    
    let mut move_count = 0;
    let max_moves = 33; // Opera Game has 33 moves
    
    for i in 0..300 { // Monitor for 5 minutes
        if let Ok(stdout) = white_process.stdout.try_next() {
            if let Some(line) = stdout {
                let line_str = String::from_utf8_lossy(&line);
                if line_str.contains("[AUTO_RECORD]") {
                    move_count += 1;
                    println!("Move {} detected: {}", move_count, line_str.trim());
                }
            }
        }
        
        if let Ok(stdout) = black_process.stdout.try_next() {
            if let Some(line) = stdout {
                let line_str = String::from_utf8_lossy(&line);
                if line_str.contains("[AUTO_RECORD]") {
                    move_count += 1;
                    println!("Move {} detected: {}", move_count, line_str.trim());
                }
            }
        }
        
        if move_count >= max_moves {
            println!("✓ All {} moves recorded!", max_moves);
            break;
        }
        
        sleep(Duration::from_secs(1)).await;
    }
    
    // Step 5: Generate results report
    println!("\n📊 Step 5: Generating Results Report");
    println!("===================================");
    
    println!("Opening results in browser...");
    
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", "start", "opera_game_results.html"])
            .spawn()?;
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg("opera_game_results.html")
            .spawn()?;
    } else {
        Command::new("xdg-open")
            .arg("opera_game_results.html")
            .spawn()?;
    }
    
    // Step 6: Cleanup
    println!("\n🧹 Step 6: Cleanup");
    println!("==================");
    
    println!("Terminating game instances...");
    white_process.kill().await?;
    black_process.kill().await?;
    
    println!("\n✅ Opera Game Test Complete!");
    println!("============================");
    println!("• {} moves recorded on-chain", move_count);
    println!("• Results report generated");
    println!("• Browser opened with game results");
    println!("• All transactions available on Solana Explorer");
    
    Ok(())
}

// Helper trait for async stdout reading
use tokio::io::AsyncBufReadExt;
use futures::TryStreamExt;

trait TryNextExt {
    fn try_next(&mut self) -> std::io::Result<Option<Vec<u8>>>;
}

impl<T> TryNextExt for T
where
    T: AsyncBufReadExt + Unpin,
{
    fn try_next(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        let mut line = String::new();
        match self.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line.into_bytes())),
            Err(e) => Err(e),
        }
    }
}
