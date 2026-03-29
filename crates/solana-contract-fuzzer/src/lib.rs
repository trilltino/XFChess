//! Solana Contract Fuzzer
//!
//! Property-based fuzzing for the XFChess Solana smart contracts.
//! Uses proptest to generate random instruction sequences and executes
//! them against devnet using user-provided funded keypairs.

pub mod strategies;
pub mod runner;
pub mod controller;
pub mod invariants;
pub mod funding;

use anyhow::Result;
use solana_sdk::signature::Keypair;
use std::path::PathBuf;

/// Fuzzer configuration
#[derive(Debug, Clone)]
pub struct FuzzerConfig {
    /// RPC endpoint (devnet)
    pub rpc_url: String,
    /// Path to master keypair file (funded)
    pub master_keypair_path: PathBuf,
    /// Number of test accounts to derive from master
    pub num_test_accounts: usize,
    /// Minimum SOL per test account
    pub min_sol_per_account: f64,
    /// Number of fuzz iterations
    pub iterations: u64,
    /// Random seed (optional)
    pub seed: Option<u64>,
    /// Enable JSON controller interface
    pub enable_controller: bool,
    /// Controller port (TCP mode)
    pub controller_port: u16,
}

impl Default for FuzzerConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.devnet.solana.com".to_string(),
            master_keypair_path: PathBuf::from("~/.config/solana/id.json"),
            num_test_accounts: 10,
            min_sol_per_account: 0.1,
            iterations: 1000,
            seed: None,
            enable_controller: false,
            controller_port: 4445,
        }
    }
}

/// Main fuzzer engine
pub struct FuzzEngine {
    pub config: FuzzerConfig,
    pub runner: runner::DevnetRunner,
}

impl FuzzEngine {
    pub async fn new(config: FuzzerConfig) -> Result<Self> {
        let runner = runner::DevnetRunner::new(&config).await?;
        Ok(Self { config, runner })
    }

    pub async fn run(&mut self) -> Result<FuzzResults> {
        // TODO: Implement fuzzing loop
        todo!()
    }
}

/// Fuzzing results summary
#[derive(Debug, Default)]
pub struct FuzzResults {
    pub iterations_completed: u64,
    pub iterations_failed: u64,
    pub invariants_violated: Vec<String>,
    pub transactions_sent: u64,
    pub transactions_failed: u64,
}
