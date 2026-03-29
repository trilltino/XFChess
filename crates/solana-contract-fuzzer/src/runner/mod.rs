//! Devnet test runner
//!
//! Executes fuzzing sequences against devnet using real transactions.

use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::sync::Arc;

use crate::{funding::FundingManager, FuzzerConfig};
use crate::strategies::FuzzInstruction;

/// Runs tests against devnet
pub struct DevnetRunner {
    pub funding: FundingManager,
    pub rpc: Arc<RpcClient>,
    pub config: FuzzerConfig,
    pub game_id_counter: u64,
}

impl DevnetRunner {
    pub async fn new(config: &FuzzerConfig) -> Result<Self> {
        let funding = FundingManager::new(
            &config.master_keypair_path,
            &config.rpc_url,
            config.num_test_accounts,
            config.min_sol_per_account,
        )?;

        // Fund all accounts before starting
        funding.fund_all_accounts().await?;

        let rpc = Arc::new(RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        ));

        Ok(Self {
            funding,
            rpc,
            config: config.clone(),
            game_id_counter: 1,
        })
    }

    /// Get next unique game ID
    pub fn next_game_id(&mut self) -> u64 {
        let id = self.game_id_counter;
        self.game_id_counter += 1;
        id
    }

    /// Execute a single fuzz instruction
    pub async fn execute(&mut self, instruction: &FuzzInstruction) -> Result<ExecutionResult> {
        match instruction {
            FuzzInstruction::CreateGame { game_id, wager, game_type, player_idx } => {
                self.create_game(*game_id, *wager, *game_type, *player_idx).await
            }
            FuzzInstruction::JoinGame { game_id, player_idx } => {
                self.join_game(*game_id, *player_idx).await
            }
            FuzzInstruction::RecordMove { game_id, move_str, fen, player_idx } => {
                self.record_move(*game_id, move_str, fen, *player_idx).await
            }
            FuzzInstruction::FinalizeGame { game_id, result, player_idx } => {
                self.finalize_game(*game_id, *result, *player_idx).await
            }
            FuzzInstruction::WithdrawExpired { game_id, player_idx } => {
                self.withdraw_expired(*game_id, *player_idx).await
            }
            FuzzInstruction::AuthorizeSession { game_id, session_pubkey, player_idx } => {
                self.authorize_session(*game_id, session_pubkey, *player_idx).await
            }
            FuzzInstruction::DelegateGame { game_id, player_idx } => {
                self.delegate_game(*game_id, *player_idx).await
            }
            FuzzInstruction::UndelegateGame { game_id, player_idx } => {
                self.undelegate_game(*game_id, *player_idx).await
            }
        }
    }

    async fn create_game(
        &self,
        game_id: u64,
        wager: u64,
        game_type: xfchess_game::state::GameType,
        player_idx: usize,
    ) -> Result<ExecutionResult> {
        // Get player keypair - clone to avoid borrow issues
        // Note: Keypair doesn't implement Clone, so we access directly
        let player_keypair_ref = self.funding.get_account(player_idx)
            .context("Invalid player index")?;

        let program_id = xfchess_game::ID;
        let blockhash = self.rpc.get_latest_blockhash()?;

        // Build instruction using solana-chess-client or manual construction
        let ix = crate::strategies::build_create_game_ix(
            program_id,
            player_keypair_ref.pubkey(),
            game_id,
            wager,
            game_type,
        )?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&player_keypair_ref.pubkey()),
            &[player_keypair_ref],
            blockhash,
        );

        match self.rpc.send_and_confirm_transaction(&tx) {
            Ok(sig) => {
                tracing::info!("Created game {}: {}", game_id, sig);
                Ok(ExecutionResult::Success(sig.to_string()))
            }
            Err(e) => {
                tracing::warn!("Failed to create game {}: {}", game_id, e);
                Ok(ExecutionResult::Failed(e.to_string()))
            }
        }
    }

    async fn join_game(&self, game_id: u64, player_idx: usize) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn record_move(
        &self,
        game_id: u64,
        move_str: &str,
        fen: &str,
        player_idx: usize,
    ) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn finalize_game(
        &self,
        game_id: u64,
        result: u8,
        player_idx: usize,
    ) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn withdraw_expired(&self, _game_id: u64, _player_idx: usize) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn authorize_session(
        &self,
        _game_id: u64,
        _session_pubkey: &Pubkey,
        _player_idx: usize,
    ) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn delegate_game(&self, _game_id: u64, _player_idx: usize) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    async fn undelegate_game(&self, _game_id: u64, _player_idx: usize) -> Result<ExecutionResult> {
        // TODO: Implement
        Ok(ExecutionResult::Skipped)
    }

    /// Get account balance
    pub fn get_balance(&self, account_idx: usize) -> Result<u64> {
        let account = self.funding.get_account(account_idx)
            .context("Invalid account index")?;
        Ok(self.rpc.get_balance(&(*account).pubkey())?)
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Success(String), // Transaction signature
    Failed(String),  // Error message
    Skipped,
}
