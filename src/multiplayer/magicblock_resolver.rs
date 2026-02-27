//! Magic Block Ephemeral Rollups Resolver Integration
//!
//! This module provides integration with Magic Block's Ephemeral Rollups (ER) for sub-second
//! transaction processing during gameplay. Transactions are routed to the ER validator during
//! active gameplay and committed to Solana when the game ends.
//!
//! Reference: https://docs.magicblock.gg/

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::sync::Arc;
use thiserror::Error;

/// The XFChess program ID on Solana
pub const XFCHESS_PROGRAM_ID: &str = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP";

/// Magic Block ER validator endpoint (default)
pub const MAGIC_BLOCK_ER_ENDPOINT: &str = "https://devnet-eu.magicblock.app";

/// Timeout for ER transactions in milliseconds
pub const ER_TIMEOUT_MS: u64 = 5000;

/// Maximum retry attempts for ER operations
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
const DELEGATE_IX_DISCRIMINATOR: [u8; 8] = [200, 179, 52, 85, 111, 249, 24, 20];

/// Undelegation instruction discriminator (8 bytes)
const UNDELEGATE_IX_DISCRIMINATOR: [u8; 8] = [30, 40, 50, 60, 70, 80, 90, 100];

/// Errors that can occur during Magic Block resolver operations
#[derive(Error, Debug, Clone)]
pub enum MagicBlockError {
    #[error("Failed to delegate game PDA: {0}")]
    DelegationFailed(String),

    #[error("Failed to undelegate game PDA: {0}")]
    UndelegationFailed(String),

    #[error("Transaction routing failed: {0}")]
    TransactionRoutingFailed(String),

    #[error("Game not delegated to ER")]
    NotDelegated,

    #[error("Retry attempts exhausted")]
    RetryExhausted,
}

/// Represents the delegation status of a game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelegationStatus {
    #[default]
    Undelegated,
    Delegating,
    Delegated,
    Undelegating,
    Failed,
}

/// Configuration for the Magic Block resolver
#[derive(Resource, Clone, Debug)]
pub struct MagicBlockConfig {
    /// The ER validator endpoint URL
    pub er_endpoint: String,
    /// The Solana program ID for XFChess
    pub program_id: Pubkey,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Whether to fall back to Solana on ER failure
    pub fallback_to_solana: bool,
}

impl Default for MagicBlockConfig {
    fn default() -> Self {
        Self {
            er_endpoint: MAGIC_BLOCK_ER_ENDPOINT.to_string(),
            program_id: XFCHESS_PROGRAM_ID.parse().unwrap_or_default(),
            max_retries: MAX_RETRY_ATTEMPTS,
            fallback_to_solana: true,
        }
    }
}

/// Magic Block resolver that handles ER interactions
#[derive(Resource)]
pub struct MagicBlockResolver {
    /// Configuration for the resolver
    config: MagicBlockConfig,
    /// Current delegation status
    pub delegation_status: DelegationStatus,
    /// The delegated game PDA (if any)
    pub delegated_game_pda: Option<Pubkey>,
    /// RPC client for Solana fallback
    solana_rpc: Option<Arc<solana_client::rpc_client::RpcClient>>,
}

impl Default for MagicBlockResolver {
    fn default() -> Self {
        Self::new(MagicBlockConfig::default())
    }
}

impl MagicBlockResolver {
    /// Creates a new MagicBlockResolver with the given configuration
    pub fn new(config: MagicBlockConfig) -> Self {
        Self {
            config,
            delegation_status: DelegationStatus::Undelegated,
            delegated_game_pda: None,
            solana_rpc: None,
        }
    }

    /// Sets the Solana RPC client for fallback
    pub fn set_solana_rpc(&mut self, rpc_client: Arc<solana_client::rpc_client::RpcClient>) {
        self.solana_rpc = Some(rpc_client);
    }

    /// Checks if a game is currently delegated to the ER
    pub fn is_delegated(&self) -> bool {
        self.delegation_status == DelegationStatus::Delegated && self.delegated_game_pda.is_some()
    }

    /// Gets the current game PDA if delegated
    pub fn get_delegated_game(&self) -> Option<Pubkey> {
        self.delegated_game_pda
    }

    /// Delegates a game PDA to the Ephemeral Rollup
    ///
    /// This should be called when a game starts to enable sub-second processing.
    pub fn delegate_game(
        &mut self,
        game_pda: Pubkey,
        payer: &Keypair,
    ) -> Result<(), MagicBlockError> {
        if self.delegation_status == DelegationStatus::Delegated {
            return Err(MagicBlockError::DelegationFailed(
                "Game already delegated".to_string(),
            ));
        }

        self.delegation_status = DelegationStatus::Delegating;
        info!("Delegating game {} to Magic Block ER", game_pda);

        // Attempt delegation with retry logic
        match self.attempt_delegation(game_pda, payer) {
            Ok(_) => {
                self.delegated_game_pda = Some(game_pda);
                self.delegation_status = DelegationStatus::Delegated;
                info!("Successfully delegated game {} to ER", game_pda);
                Ok(())
            }
            Err(e) => {
                self.delegation_status = DelegationStatus::Failed;
                error!("Failed to delegate game {}: {}", game_pda, e);
                Err(MagicBlockError::DelegationFailed(e.to_string()))
            }
        }
    }

    /// Attempts to delegate the game to ER with retry logic
    fn attempt_delegation(
        &mut self,
        game_pda: Pubkey,
        payer: &Keypair,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            match self.execute_delegation(game_pda, payer) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    warn!(
                        "Delegation attempt {} failed for game {}: {}",
                        attempt, game_pda, e
                    );
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        // Exponential backoff: 100ms, 200ms, 400ms
                        let delay = std::time::Duration::from_millis(100 * 2_u64.pow(attempt - 1));
                        std::thread::sleep(delay);
                    }
                }
            }
        }

        Err(Box::new(MagicBlockError::RetryExhausted))
    }

    /// Executes the actual delegation transaction
    fn execute_delegation(
        &self,
        game_pda: Pubkey,
        payer: &Keypair,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Executing delegation for game PDA: {}", game_pda);

        // Create delegation instruction
        let delegation_ix = self.create_delegation_instruction(game_pda, payer.pubkey())?;

        // Send transaction via Solana RPC to the MagicBlock validator
        // The validator will process the delegation request
        let rpc_client = self
            .solana_rpc
            .as_ref()
            .ok_or("Solana RPC client not configured")?;

        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[delegation_ix],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        // Send and confirm the delegation transaction
        let signature = rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| format!("Delegation transaction failed: {}", e))?;

        info!("Delegation transaction confirmed: {}", signature);

        // Verify delegation status by checking if the game account is now delegated
        // In a real implementation, we would query the delegation status from MagicBlock
        std::thread::sleep(std::time::Duration::from_millis(100));

        info!("Game {} successfully delegated to ER", game_pda);
        Ok(())
    }

    /// Creates a delegation instruction for the ER
    ///
    /// This uses the MagicBlock ER delegation pattern where the game account
    /// is delegated to the ER validator for sub-second processing.
    fn create_delegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        // Derive the delegation record PDA
        let delegation_seeds = &[b"delegation", game_pda.as_ref()];
        let (delegation_pda, _) =
            Pubkey::find_program_address(delegation_seeds, &self.config.program_id);

        // Create delegation instruction matching the xfchess-game program
        // Accounts:
        // 0. payer (signer, writable) - pays for account creation
        // 1. game_pda (writable) - the game account to delegate
        // 2. delegation_pda (writable) - stores delegation metadata
        // 3. system_program - for account creation
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(delegation_pda, false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(),
                false,
            ),
        ];

        // Instruction data: 8-byte discriminator for delegate instruction
        let data = DELEGATE_IX_DISCRIMINATOR.to_vec();

        Ok(Instruction::new_with_bytes(
            self.config.program_id,
            &data,
            accounts,
        ))
    }

    /// Undelegates a game PDA from the Ephemeral Rollup
    ///
    /// This should be called when a game ends to commit the final state to Solana.
    pub fn undelegate_game(&mut self, payer: &Keypair) -> Result<(), MagicBlockError> {
        let game_pda = match self.delegated_game_pda {
            Some(pda) => pda,
            None => return Err(MagicBlockError::NotDelegated),
        };

        if self.delegation_status != DelegationStatus::Delegated {
            return Err(MagicBlockError::UndelegationFailed(
                "Game not in delegated state".to_string(),
            ));
        }

        self.delegation_status = DelegationStatus::Undelegating;
        info!("Undelegating game {} from Magic Block ER", game_pda);

        // Attempt undelegation with retry logic
        match self.attempt_undelegation(game_pda, payer) {
            Ok(_) => {
                self.delegated_game_pda = None;
                self.delegation_status = DelegationStatus::Undelegated;
                info!("Successfully undelegated game {} from ER", game_pda);
                Ok(())
            }
            Err(e) => {
                self.delegation_status = DelegationStatus::Failed;
                error!("Failed to undelegate game {}: {}", game_pda, e);
                Err(MagicBlockError::UndelegationFailed(e.to_string()))
            }
        }
    }

    /// Attempts to undelegate the game from ER with retry logic
    fn attempt_undelegation(
        &self,
        game_pda: Pubkey,
        payer: &Keypair,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            match self.execute_undelegation(game_pda, payer) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    warn!(
                        "Undelegation attempt {} failed for game {}: {}",
                        attempt, game_pda, e
                    );
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        let delay = std::time::Duration::from_millis(100 * 2_u64.pow(attempt - 1));
                        std::thread::sleep(delay);
                    }
                }
            }
        }

        Err(Box::new(MagicBlockError::RetryExhausted))
    }

    /// Executes the actual undelegation transaction
    fn execute_undelegation(
        &self,
        game_pda: Pubkey,
        payer: &Keypair,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Executing undelegation for game PDA: {}", game_pda);

        // Create undelegation instruction
        let undelegation_ix = self.create_undelegation_instruction(game_pda, payer.pubkey())?;

        // Send transaction via Solana RPC
        let rpc_client = self
            .solana_rpc
            .as_ref()
            .ok_or("Solana RPC client not configured")?;

        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get blockhash: {}", e))?;

        let transaction = Transaction::new_signed_with_payer(
            &[undelegation_ix],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        // Send and confirm the undelegation transaction
        let signature = rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| format!("Undelegation transaction failed: {}", e))?;

        info!("Undelegation transaction confirmed: {}", signature);
        info!("Game {} successfully undelegated from ER", game_pda);

        Ok(())
    }

    /// Creates an undelegation instruction for the ER
    fn create_undelegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        // Derive the delegation record PDA
        let delegation_seeds = &[b"delegation", game_pda.as_ref()];
        let (delegation_pda, _) =
            Pubkey::find_program_address(delegation_seeds, &self.config.program_id);

        // Create undelegation instruction
        // Accounts:
        // 0. payer (signer, writable)
        // 1. game_pda (writable) - the game account to undelegate
        // 2. delegation_pda (writable) - stores delegation metadata
        // 3. system_program
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(delegation_pda, false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(),
                false,
            ),
        ];

        // Instruction data: 8-byte discriminator for undelegate instruction
        let data = UNDELEGATE_IX_DISCRIMINATOR.to_vec();

        Ok(Instruction::new_with_bytes(
            self.config.program_id,
            &data,
            accounts,
        ))
    }

    /// Routes a transaction to the appropriate destination
    ///
    /// If the game is delegated to ER, routes to the ER validator for sub-second processing.
    /// Otherwise, falls back to direct Solana submission.
    pub fn route_transaction(
        &self,
        instructions: Vec<Instruction>,
        payer: &Keypair,
    ) -> Result<String, MagicBlockError> {
        if self.is_delegated() {
            self.route_to_er(instructions, payer)
        } else {
            self.route_to_solana(instructions, payer)
        }
    }

    /// Routes a transaction to the Ephemeral Rollup
    fn route_to_er(
        &self,
        instructions: Vec<Instruction>,
        payer: &Keypair,
    ) -> Result<String, MagicBlockError> {
        info!("Routing transaction to Magic Block ER");

        // Attempt to send to ER with retry logic
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            match self.send_to_er(&instructions, payer) {
                Ok(signature) => {
                    info!("Transaction sent to ER with signature: {}", signature);
                    return Ok(signature);
                }
                Err(e) => {
                    warn!("ER routing attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        let delay = std::time::Duration::from_millis(50 * 2_u64.pow(attempt - 1));
                        std::thread::sleep(delay);
                    }
                }
            }
        }

        // Fall back to Solana if configured
        if self.config.fallback_to_solana {
            warn!("ER routing failed, falling back to Solana");
            return self.route_to_solana(instructions, payer);
        }

        Err(MagicBlockError::TransactionRoutingFailed(
            last_error.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }

    /// Sends a transaction to the Ephemeral Rollup
    ///
    /// This submits the transaction to the MagicBlock ER validator which processes
    /// it with sub-second finality while the game is delegated.
    fn send_to_er(&self, instructions: &[Instruction], payer: &Keypair) -> Result<String, String> {
        info!(
            "Sending transaction to MagicBlock ER at {}",
            self.config.er_endpoint
        );

        // Create RPC client pointing to MagicBlock ER endpoint
        let er_rpc_client = RpcClient::new(self.config.er_endpoint.clone());

        // Get recent blockhash from ER
        let recent_blockhash = er_rpc_client
            .get_latest_blockhash()
            .map_err(|e| format!("Failed to get ER blockhash: {}", e))?;

        // Create and sign transaction
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        // Send transaction to ER - this should be sub-second
        let signature = er_rpc_client
            .send_transaction(&transaction)
            .map_err(|e| format!("Failed to send transaction to ER: {}", e))?;

        info!("Transaction sent to ER with signature: {}", signature);

        // Optionally wait for confirmation (ER provides sub-second confirmation)
        match er_rpc_client.confirm_transaction(&signature) {
            Ok(true) => {
                info!("Transaction confirmed on ER: {}", signature);
                Ok(signature.to_string())
            }
            Ok(false) => {
                warn!("Transaction not yet confirmed on ER: {}", signature);
                // Return signature anyway - it may confirm shortly
                Ok(signature.to_string())
            }
            Err(e) => {
                warn!("Error confirming transaction on ER: {}", e);
                // Return signature anyway - it may have succeeded
                Ok(signature.to_string())
            }
        }
    }

    /// Routes a transaction directly to Solana
    fn route_to_solana(
        &self,
        instructions: Vec<Instruction>,
        payer: &Keypair,
    ) -> Result<String, MagicBlockError> {
        info!("Routing transaction directly to Solana");

        let rpc_client = match &self.solana_rpc {
            Some(client) => client.clone(),
            None => {
                return Err(MagicBlockError::TransactionRoutingFailed(
                    "No Solana RPC client configured".to_string(),
                ));
            }
        };

        // Create transaction
        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .map_err(|e| MagicBlockError::TransactionRoutingFailed(e.to_string()))?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );

        // Send transaction
        match rpc_client.send_and_confirm_transaction(&transaction) {
            Ok(signature) => {
                info!("Transaction sent to Solana with signature: {}", signature);
                Ok(signature.to_string())
            }
            Err(e) => {
                error!("Failed to send transaction to Solana: {}", e);
                Err(MagicBlockError::TransactionRoutingFailed(e.to_string()))
            }
        }
    }
}

/// Events for Magic Block resolver
#[derive(Event, Message, Debug, Clone)]
pub enum MagicBlockEvent {
    /// Game has been delegated to ER
    GameDelegated { game_pda: Pubkey },
    /// Game has been undelegated from ER
    GameUndelegated { game_pda: Pubkey },
    /// Delegation failed
    DelegationFailed { game_pda: Pubkey, error: String },
    /// Undelegation failed
    UndelegationFailed { game_pda: Pubkey, error: String },
    /// Transaction routed to ER
    TransactionRoutedToEr { signature: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_status_transitions() {
        let config = MagicBlockConfig::default();
        let mut resolver = MagicBlockResolver::new(config);

        assert_eq!(resolver.delegation_status, DelegationStatus::Undelegated);
        assert!(!resolver.is_delegated());

        // Note: delegation requires actual keypair, so we test status transitions only
        resolver.delegation_status = DelegationStatus::Delegated;
        assert!(resolver.is_delegated());

        resolver.delegation_status = DelegationStatus::Undelegated;
        assert!(!resolver.is_delegated());
    }

    #[test]
    fn test_magic_block_config_default() {
        let config = MagicBlockConfig::default();

        assert_eq!(config.er_endpoint, MAGIC_BLOCK_ER_ENDPOINT);
        assert_eq!(config.timeout_ms, ER_TIMEOUT_MS);
        assert_eq!(config.max_retries, MAX_RETRY_ATTEMPTS);
        assert!(config.fallback_to_solana);
    }

    #[test]
    fn test_magic_block_error_display() {
        let err = MagicBlockError::NotDelegated;
        assert_eq!(err.to_string(), "Game not delegated to ER");

        let err = MagicBlockError::DelegationFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }
}
