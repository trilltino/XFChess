//! Account funding system
//!
//! Derives test accounts from a master keypair and distributes SOL
//! to ensure all accounts can participate in wager games.

use anyhow::{Context, Result};
use ed25519_dalek::{PublicKey, SecretKey, Keypair as DalekKeypair};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
    commitment_config::CommitmentConfig,
};
use std::path::Path;

/// Manages funding for fuzz test accounts
pub struct FundingManager {
    /// Master keypair (user-provided, pre-funded on devnet)
    pub master: Keypair,
    /// Derived test accounts
    pub test_accounts: Vec<Keypair>,
    /// Minimum balance required per account (lamports)
    pub min_balance_lamports: u64,
    /// RPC client
    pub rpc: RpcClient,
}

impl FundingManager {
    /// Load master keypair from file and derive test accounts
    pub fn new(
        master_keypair_path: &Path,
        rpc_url: &str,
        num_test_accounts: usize,
        min_sol_per_account: f64,
    ) -> Result<Self> {
        // Load master keypair
        let master = load_keypair_from_file(master_keypair_path)
            .context("Failed to load master keypair")?;

        let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

        // Derive test accounts deterministically from master
        let test_accounts = derive_test_accounts(&master, num_test_accounts);

        let min_balance_lamports = (min_sol_per_account * 1_000_000_000.0) as u64;

        Ok(Self {
            master,
            test_accounts,
            min_balance_lamports,
            rpc,
        })
    }

    /// Check and fund all test accounts
    pub async fn fund_all_accounts(&self) -> Result<()> {
        let master_balance = self.rpc.get_balance(&self.master.pubkey())?;
        tracing::info!("Master account balance: {} SOL", master_balance as f64 / 1_000_000_000.0);

        if master_balance < self.min_balance_lamports * self.test_accounts.len() as u64 {
            anyhow::bail!(
                "Master account underfunded. Need at least {} SOL, have {} SOL",
                (self.min_balance_lamports * self.test_accounts.len() as u64) as f64 / 1_000_000_000.0,
                master_balance as f64 / 1_000_000_000.0
            );
        }

        for (i, account) in self.test_accounts.iter().enumerate() {
            self.ensure_funded(account, i).await?;
        }

        tracing::info!("All {} test accounts funded", self.test_accounts.len());
        Ok(())
    }

    /// Ensure a single account has minimum balance
    async fn ensure_funded(&self, account: &Keypair, index: usize) -> Result<()> {
        let balance = self.rpc.get_balance(&account.pubkey())?;
        
        if balance >= self.min_balance_lamports {
            tracing::debug!("Account {} already funded with {} lamports", index, balance);
            return Ok(());
        }

        let needed = self.min_balance_lamports.saturating_sub(balance);
        tracing::info!("Funding account {} with {} lamports", index, needed);

        let blockhash = self.rpc.get_latest_blockhash()?;
        let transfer_ix = system_instruction::transfer(
            &self.master.pubkey(),
            &account.pubkey(),
            needed + 2_000_000, // Extra for transaction fees
        );

        let tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&self.master.pubkey()),
            &[&self.master],
            blockhash,
        );

        self.rpc.send_and_confirm_transaction(&tx)
            .with_context(|| format!("Failed to fund account {}", index))?;

        Ok(())
    }

    /// Get a random test account (for fuzzing)
    pub fn random_account(&self) -> &Keypair {
        use rand::seq::SliceRandom;
        self.test_accounts
            .choose(&mut rand::thread_rng())
            .expect("At least one test account")
    }

    /// Get account by index
    pub fn get_account(&self, index: usize) -> Option<&Keypair> {
        self.test_accounts.get(index)
    }

    /// Get master keypair
    pub fn master(&self) -> &Keypair {
        &self.master
    }
}

/// Load keypair from JSON file (standard Solana format)
fn load_keypair_from_file(path: &Path) -> Result<Keypair> {
    let expanded = if path.starts_with("~") {
        let home = dirs::home_dir().context("Could not find home directory")?;
        home.join(path.strip_prefix("~")?)
    } else {
        path.to_path_buf()
    };

    let contents = std::fs::read_to_string(&expanded)
        .with_context(|| format!("Failed to read keypair from {:?}", expanded))?;

    let bytes: Vec<u8> = serde_json::from_str(&contents)
        .context("Invalid keypair JSON format")?;

    Keypair::from_bytes(&bytes)
        .context("Invalid keypair bytes")
}

/// Derive test accounts deterministically from master seed
fn derive_test_accounts(master: &Keypair, count: usize) -> Vec<Keypair> {
    let mut accounts = Vec::with_capacity(count);
    let master_bytes = master.to_bytes();

    for i in 0..count {
        // Deterministic derivation using hash of master + index
        let mut seed = [0u8; 32];
        let hash_input = [&master_bytes[..], &i.to_le_bytes()].concat();
        let hash = blake3::hash(&hash_input);
        seed.copy_from_slice(hash.as_bytes());

        let secret = SecretKey::from_bytes(&seed)
            .expect("Valid seed");
        let public = PublicKey::from(&secret);
        let dalek_kp = DalekKeypair { secret, public };
        
        // Convert to solana-sdk Keypair
        let solana_kp = Keypair::from_bytes(&dalek_kp.to_bytes())
            .expect("Valid conversion");
        
        accounts.push(solana_kp);
    }

    accounts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_deterministic() {
        let master = Keypair::new();
        let accounts1 = derive_test_accounts(&master, 5);
        let accounts2 = derive_test_accounts(&master, 5);
        
        // Should produce same accounts
        for (a1, a2) in accounts1.iter().zip(accounts2.iter()) {
            assert_eq!(a1.pubkey(), a2.pubkey());
        }
    }
}
