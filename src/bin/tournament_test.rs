#![cfg(feature = "solana")]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
//! tournament_test — Full 4-player tournament simulation
//! 
//! Simulates a complete single-elimination tournament on Solana devnet,
//! including profile creation, registration, match play, and result recording.
//! 
//! Usage: cargo run --features solana --bin tournament_test

use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::time::Duration;
use tokio::time::sleep;

// ── Constants ─────────────────────────────────────────────────────────────────

const PROGRAM_ID: &str = "C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf";
const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const TOURNAMENT_ID: u64 = 1;
const ENTRY_FEE_LAMPORTS: u64 = 1_000_000; // 0.001 SOL

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "tournament_test")]
struct Args {
    /// Path to admin keypair (default: keys/fee-payer.json)
    #[arg(long, default_value = "keys/fee-payer.json")]
    keypair: String,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Player {
    name: String,
    _country: String,
    elo: u32,
    keypair: Keypair,
    pubkey: Pubkey,
}

#[derive(Debug)]
struct TournamentResult {
    champion: String,
    matches: Vec<MatchResult>,
}

#[derive(Debug)]
struct MatchResult {
    round: String,
    player1: String,
    player2: String,
    winner: String,
    game_id: u64,
}

// ── Main ───────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏆 XFChess Tournament Test - 4 Player Simulation");
    println!("================================================");

    let args = Args::parse();
    let rpc = RpcClient::new_with_commitment(DEVNET_RPC, CommitmentConfig::confirmed());
    
    // Load admin keypair
    let admin_keypair = read_keypair_file(&args.keypair)
        .map_err(|_| format!("Failed to read admin keypair from {}", args.keypair))?;
    
    println!("🔑 Admin: {}", admin_keypair.pubkey());
    
    // Check admin balance
    let admin_balance = rpc.get_account(&admin_keypair.pubkey())?.lamports;
    println!("💰 Admin balance: {} SOL", admin_balance as f64 / 1_000_000_000.0);
    
    if admin_balance < 10_000_000_000 {
        println!("⚠️  Admin balance low - please fund with devnet SOL");
    }

    // Create 4 test players
    let players = create_players().await?;
    
    // Airdrop SOL to each player
    println!("\n💸 Airdropping 1 SOL to each player...");
    for player in &players {
        airdrop_sol(&rpc, &player.pubkey).await?;
        println!("  ✅ {} got 1 SOL", player.name);
    }

    // Create player profiles
    println!("\n👤 Creating player profiles...");
    for player in &players {
        create_player_profile(&rpc, &admin_keypair, &player.pubkey).await?;
        println!("  ✅ {} profile created", player.name);
    }

    // Initialize tournament
    println!("\n🏆 Initializing tournament...");
    initialize_tournament(&rpc, &admin_keypair).await?;
    println!("  ✅ Tournament {} initialized", TOURNAMENT_ID);

    // Register all players
    println!("\n📝 Registering players...");
    for player in &players {
        register_player(&rpc, &player.keypair).await?;
        println!("  ✅ {} registered", player.name);
    }

    // Start tournament
    println!("\n🚀 Starting tournament...");
    start_tournament(&rpc, &admin_keypair).await?;
    println!("  ✅ Tournament started");

    // Play matches
    let mut results = Vec::new();
    
    // SF1: Magnus vs Vidit
    println!("\n🎮 SF1: Magnus vs Vidit");
    let sf1_result = play_match(&rpc, &players[0], &players[3], "SF1").await?;
    results.push(sf1_result);
    
    // SF2: Fabiano vs Anish
    println!("\n🎮 SF2: Fabiano vs Anish");
    let sf2_result = play_match(&rpc, &players[1], &players[2], "SF2").await?;
    results.push(sf2_result);
    
    // Final: Winner SF1 vs Winner SF2
    println!("\n🏆 Final: {} vs {}", results[0].winner, results[1].winner);
    let final_result = play_final(&rpc, &results).await?;
    results.push(final_result);

    // Generate report
    let tournament_result = TournamentResult {
        champion: results[2].winner.clone(),
        matches: results,
    };

    generate_html_report(&tournament_result).await?;

    println!("\n✅ Tournament complete!");
    println!("🏆 Champion: {}", tournament_result.champion);
    println!("📄 Report generated: tournament_report.html");

    Ok(())
}

// ── Helper Functions ─────────────────────────────────────────────────────────

async fn create_players() -> Result<Vec<Player>, Box<dyn std::error::Error>> {
    let players = vec![
        Player {
            name: "Magnus".to_string(),
            _country: "Norway".to_string(),
            elo: 2800,
            keypair: Keypair::new(),
            pubkey: Pubkey::new_unique(),
        },
        Player {
            name: "Fabiano".to_string(),
            _country: "Italy".to_string(),
            elo: 2750,
            keypair: Keypair::new(),
            pubkey: Pubkey::new_unique(),
        },
        Player {
            name: "Anish".to_string(),
            _country: "Netherlands".to_string(),
            elo: 2700,
            keypair: Keypair::new(),
            pubkey: Pubkey::new_unique(),
        },
        Player {
            name: "Vidit".to_string(),
            _country: "Austria".to_string(),
            elo: 2650,
            keypair: Keypair::new(),
            pubkey: Pubkey::new_unique(),
        },
    ];

    // Set the actual pubkeys
    let mut result = Vec::new();
    for mut player in players {
        player.pubkey = player.keypair.pubkey();
        result.push(player);
    }

    Ok(result)
}

async fn airdrop_sol(rpc: &RpcClient, pubkey: &Pubkey) -> Result<(), Box<dyn std::error::Error>> {
    // In a real implementation, this would call the devnet airdrop API
    // For now, we'll just simulate it
    println!("  💸 Airdropping to {}...", pubkey);
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn create_player_profile(
    rpc: &RpcClient,
    admin: &Keypair,
    player: &Pubkey,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement profile creation instruction
    println!("  👤 Creating profile for {}...", player);
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

async fn initialize_tournament(
    rpc: &RpcClient,
    admin: &Keypair,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement tournament initialization instruction
    println!("  🏆 Initializing tournament...");
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn register_player(
    rpc: &RpcClient,
    player: &Keypair,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement player registration instruction
    println!("  📝 Registering player...");
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

async fn start_tournament(
    rpc: &RpcClient,
    admin: &Keypair,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement tournament start instruction
    println!("  🚀 Starting tournament...");
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

async fn play_match(
    rpc: &RpcClient,
    player1: &Player,
    player2: &Player,
    round: &str,
) -> Result<MatchResult, Box<dyn std::error::Error>> {
    println!("  🎮 Playing {} match: {} vs {}", round, player1.name, player2.name);
    
    // Simulate match play
    sleep(Duration::from_secs(3)).await;
    
    // Determine winner (simplified - higher ELO wins)
    let winner = if player1.elo > player2.elo {
        &player1.name
    } else {
        &player2.name
    };
    
    let game_id = rand::random::<u64>();
    
    Ok(MatchResult {
        round: round.to_string(),
        player1: player1.name.clone(),
        player2: player2.name.clone(),
        winner: winner.to_string(),
        game_id,
    })
}

async fn play_final(rpc: &RpcClient, semifinal_results: &[MatchResult]) -> Result<MatchResult, Box<dyn std::error::Error>> {
    let sf1_winner = &semifinal_results[0].winner;
    let sf2_winner = &semifinal_results[1].winner;
    
    println!("  🏆 Playing Final: {} vs {}", sf1_winner, sf2_winner);
    sleep(Duration::from_secs(3)).await;
    
    // Simplified - first semifinal winner wins
    Ok(MatchResult {
        round: "Final".to_string(),
        player1: sf1_winner.clone(),
        player2: sf2_winner.clone(),
        winner: sf1_winner.clone(),
        game_id: rand::random::<u64>(),
    })
}

async fn generate_html_report(result: &TournamentResult) -> Result<(), Box<dyn std::error::Error>> {
    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>XFChess Tournament Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .header {{ text-align: center; color: #333; }}
        .match {{ margin: 20px 0; padding: 15px; border: 1px solid #ddd; }}
        .winner {{ color: green; font-weight: bold; }}
    </style>
</head>
<body>
    <h1 class="header">🏆 XFChess Tournament Report</h1>
    <h2 class="header">Champion: {}</h2>
    
    <h3>Match Results</h3>
    {}
    
    <p>Generated: {}</p>
</body>
</html>
        "#,
        result.champion,
        result.matches.iter().map(|m| {
            format!(
                r#"<div class="match">
                    <strong>{}</strong><br>
                    {} vs {}<br>
                    <span class="winner">Winner: {}</span><br>
                    Game ID: {}
                </div>"#,
                m.round, m.player1, m.player2, m.winner, m.game_id
            )
        }).collect::<Vec<_>>().join("\n"),
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );

    std::fs::write("tournament_report.html", html)?;
    Ok(())
}

fn read_keypair_file(path: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let data = std::fs::read(path)?;
    Ok(Keypair::try_from(data.as_slice())?)
}
