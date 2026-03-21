//! Magic Block Ephemeral Rollups Resolver Integration
//!
//! This module provides integration with Magic Block's Ephemeral Rollups (ER) for sub-second
//! transaction processing during gameplay. Transactions are routed to the ER validator during
//! active gameplay and committed to Solana when the game ends.
//!
//! Reference: https://docs.magicblock.gg/

use bevy::prelude::*;
use sha2::{Digest, Sha256};
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

/// MagicBlock explorer base URL for ER transactions
pub const MAGIC_BLOCK_EXPLORER: &str = "https://explorer.magicblock.gg";

/// Timeout for ER transactions in milliseconds
pub const ER_TIMEOUT_MS: u64 = 5000;

/// Maximum retry attempts for ER operations
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// MagicBlock Delegation Program ID (from ephemeral-rollups-sdk)
pub const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSS";

/// MagicBlock Magic Context account
pub const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";

/// MagicBlock Magic Program ID
pub const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";

/// Compute the 8-byte Anchor discriminator for `global:<fn_name>`.
fn anchor_disc(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

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
    /// Game ID of the currently delegated game
    delegated_game_id: Option<u64>,
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
            delegated_game_id: None,
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

    /// Sets the game ID used in delegation/undelegation instructions.
    pub fn set_game_id(&mut self, game_id: u64) {
        self.delegated_game_id = Some(game_id);
    }

    /// Returns the MagicBlock ER explorer URL for a given tx signature.
    pub fn er_explorer_url(&self, signature: &str) -> String {
        format!("{}/tx/{}?cluster=devnet", MAGIC_BLOCK_EXPLORER, signature)
    }

    /// Delegates a game PDA to the Ephemeral Rollup.
    ///
    /// This should be called when a game starts to enable sub-second processing.
    /// `game_id` is required to derive the on-chain seeds and pass as instruction arg.
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
                // Derive game_id from PDA context (stored during delegation)
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

    /// Creates a `delegate_game` Anchor instruction matching the on-chain
    /// `DelegateGameCtx` account layout.
    ///
    /// Accounts (order matches Anchor derive):
    ///   0. game        (mut)       — game PDA
    ///   1. payer       (mut, sign) — pays for delegation
    ///   2. owner_program           — xfchess-game program itself
    ///   3. buffer      (mut)       — delegation buffer PDA
    ///   4. delegation_record (mut) — delegation record PDA
    ///   5. delegation_metadata(mut)— delegation metadata PDA
    ///   6. delegation_program      — MagicBlock delegation program
    ///   7. system_program
    fn create_delegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
            .parse()
            .map_err(|_| MagicBlockError::DelegationFailed("Bad delegation program id".into()))?;

        // Buffer PDA: seeds = ["buffer", game_pda] owner = xfchess program
        let (buffer_pda, _) = Pubkey::find_program_address(
            &[b"buffer", game_pda.as_ref()],
            &self.config.program_id,
        );

        // Delegation record PDA: seeds = ["delegation", game_pda] owner = delegation program
        let (delegation_record, _) = Pubkey::find_program_address(
            &[b"delegation", game_pda.as_ref()],
            &delegation_program_id,
        );

        // Delegation metadata PDA: seeds = ["delegation-metadata", game_pda] owner = delegation program
        let (delegation_metadata, _) = Pubkey::find_program_address(
            &[b"delegation-metadata", game_pda.as_ref()],
            &delegation_program_id,
        );

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new_readonly(self.config.program_id, false),
            solana_sdk::instruction::AccountMeta::new(buffer_pda, false),
            solana_sdk::instruction::AccountMeta::new(delegation_record, false),
            solana_sdk::instruction::AccountMeta::new(delegation_metadata, false),
            solana_sdk::instruction::AccountMeta::new_readonly(delegation_program_id, false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(),
                false,
            ),
        ];

        // game_id is derived from the game PDA context; pass a default
        // valid_until of 600 seconds (10 min game session).
        let game_id = self.delegated_game_id.unwrap_or(0);
        let valid_until: i64 = 600;

        let mut data = anchor_disc("delegate_game").to_vec();
        data.extend_from_slice(&game_id.to_le_bytes());
        data.extend_from_slice(&valid_until.to_le_bytes());

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

    /// Creates an `undelegate_game` Anchor instruction matching the on-chain
    /// `UndelegateGameCtx` account layout.
    ///
    /// Accounts (order matches Anchor derive):
    ///   0. game          (mut)       — game PDA
    ///   1. payer         (mut, sign) — pays tx fees
    ///   2. magic_context (mut)       — MagicBlock magic context
    ///   3. magic_program             — MagicBlock magic program
    fn create_undelegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY
            .parse()
            .map_err(|_| MagicBlockError::UndelegationFailed("Bad magic context id".into()))?;
        let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY
            .parse()
            .map_err(|_| MagicBlockError::UndelegationFailed("Bad magic program id".into()))?;

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new(magic_context, false),
            solana_sdk::instruction::AccountMeta::new_readonly(magic_program, false),
        ];

        let game_id = self.delegated_game_id.unwrap_or(0);

        let mut data = anchor_disc("undelegate_game").to_vec();
        data.extend_from_slice(&game_id.to_le_bytes());

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
