//! Solana Contract Fuzzer CLI
//!
//! Command-line interface for fuzzing the XFChess smart contracts on devnet.

use anyhow::Result;
use clap::Parser;
use solana_contract_fuzzer::{FuzzEngine, FuzzerConfig};
use std::path::PathBuf;
use tracing::{info, warn, error};

#[derive(Parser, Debug)]
#[command(name = "solana-contract-fuzzer")]
#[command(about = "Fuzz XFChess Solana smart contracts on devnet")]
struct Cli {
    /// RPC endpoint (default: devnet)
    #[arg(short, long, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,

    /// Path to funded master keypair JSON file
    #[arg(short, long, default_value = "~/.config/solana/id.json")]
    keypair: PathBuf,

    /// Number of test accounts to derive
    #[arg(short, long, default_value = "10")]
    accounts: usize,

    /// Minimum SOL per test account
    #[arg(short = 'm', long, default_value = "0.05")]
    min_sol: f64,

    /// Number of fuzz iterations
    #[arg(short, long, default_value = "1000")]
    iterations: u64,

    /// Random seed (optional, for reproducibility)
    #[arg(long)]
    seed: Option<u64>,

    /// Enable JSON controller interface
    #[arg(long)]
    controller: bool,

    /// Controller port (TCP mode)
    #[arg(short, long, default_value = "4445")]
    port: u16,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    info!("Starting Solana Contract Fuzzer");
    info!("RPC: {}", cli.rpc_url);
    info!("Master keypair: {:?}", cli.keypair);
    info!("Test accounts: {}", cli.accounts);
    info!("Iterations: {}", cli.iterations);

    // Validate min_sol is sufficient
    if cli.min_sol < 0.01 {
        warn!("Minimum SOL per account is very low ({}). Recommended: 0.05+", cli.min_sol);
    }

    // Build config
    let config = FuzzerConfig {
        rpc_url: cli.rpc_url,
        master_keypair_path: cli.keypair,
        num_test_accounts: cli.accounts,
        min_sol_per_account: cli.min_sol,
        iterations: cli.iterations,
        seed: cli.seed,
        enable_controller: cli.controller,
        controller_port: cli.port,
    };

    // Initialize fuzzer
    let mut engine = FuzzEngine::new(config).await?;

    info!("Fuzzer initialized, starting fuzz run...");

    // Run fuzzing
    let results = engine.run().await?;

    // Report results
    info!("\n=== Fuzzing Results ===");
    info!("Iterations completed: {}", results.iterations_completed);
    info!("Iterations failed: {}", results.iterations_failed);
    info!("Transactions sent: {}", results.transactions_sent);
    info!("Transactions failed: {}", results.transactions_failed);
    
    if results.invariants_violated.is_empty() {
        info!("All invariants passed!");
    } else {
        error!("Invariant violations found:");
        for violation in &results.invariants_violated {
            error!("  - {}", violation);
        }
    }

    // Exit with error code if any failures
    if results.iterations_failed > 0 || !results.invariants_violated.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
