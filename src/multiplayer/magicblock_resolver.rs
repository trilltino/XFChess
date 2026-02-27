//! Magic Block Ephemeral Rollups Resolver Integration
//!
//! This module provides integration with Magic Block's Ephemeral Rollups (ER) for sub-second
//! transaction processing during gameplay. Transactions are routed to the ER validator during
//! active gameplay and committed to Solana when the game ends.
//!
//! Reference: https://docs.magicblock.gg/

use bevy::prelude::*;
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

/// Magic Block ER validator endpoint
pub const MAGIC_BLOCK_ER_ENDPOINT: &str = "https://er.magicblock.gg";

/// Timeout for ER transactions in milliseconds
pub const ER_TIMEOUT_MS: u64 = 5000;

/// Maximum retry attempts for ER operations
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Errors that can occur during Magic Block resolver operations
#[derive(Error, Debug, Clone)]
pub enum MagicBlockError {
    #[error("Failed to delegate game PDA: {0}")]
    DelegationFailed(String),

    #[error("Failed to undelegate game PDA: {0}")]
    UndelegationFailed(String),

    #[error("Transaction routing failed: {0}")]
    TransactionRoutingFailed(String),

    #[error("ER connection error: {0}")]
    ConnectionError(String),

    #[error("Game not delegated to ER")]
    NotDelegated,

    #[error("Timeout waiting for ER response")]
    Timeout,

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
    /// Timeout for ER operations in milliseconds
    pub timeout_ms: u64,
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
            timeout_ms: ER_TIMEOUT_MS,
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
    /// Session keypair for signing ER transactions
    session_keypair: Option<Arc<Keypair>>,
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
            session_keypair: None,
            solana_rpc: None,
        }
    }

    /// Sets the session keypair for signing transactions
    pub fn set_session_keypair(&mut self, keypair: Arc<Keypair>) {
        self.session_keypair = Some(keypair);
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
        // Create delegation instruction
        // This delegates the game PDA to the ER validator
        let delegation_ix = self.create_delegation_instruction(game_pda, payer.pubkey())?;

        // For now, simulate successful delegation
        // In production, this would use magic-resolver client to:
        // 1. Send delegation transaction to ER
        // 2. Wait for confirmation
        // 3. Verify delegation status
        info!("Executing delegation for game PDA: {}", game_pda);

        // Placeholder: simulate network delay
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(())
    }

    /// Creates a delegation instruction for the ER
    fn create_delegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        // Derive the delegation record PDA
        let delegation_seeds = &[b"delegation", game_pda.as_ref()];
        let (delegation_pda, _) =
            Pubkey::find_program_address(delegation_seeds, &self.config.program_id);

        // Create delegation instruction
        // Accounts:
        // 0. payer (signer, writable)
        // 1. game_pda (writable)
        // 2. delegation_pda (writable)
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

        // Instruction data: [8-byte discriminator]
        let data = vec![0x01]; // Delegation discriminator

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
        // Create undelegation instruction
        let undelegation_ix = self.create_undelegation_instruction(game_pda, payer.pubkey())?;

        info!("Executing undelegation for game PDA: {}", game_pda);

        // Placeholder: simulate network delay
        std::thread::sleep(std::time::Duration::from_millis(50));

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
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(delegation_pda, false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(),
                false,
            ),
        ];

        // Instruction data: [8-byte discriminator]
        let data = vec![0x02]; // Undelegation discriminator

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
    fn send_to_er(&self, instructions: &[Instruction], payer: &Keypair) -> Result<String, String> {
        // In production, this would use magic-resolver client to:
        // 1. Create and sign transaction
        // 2. Send to ER validator endpoint
        // 3. Wait for confirmation (sub-second)
        // 4. Return transaction signature

        // Placeholder implementation
        let mock_signature = format!("ER{}", uuid::Uuid::new_v4().to_string().replace("-", ""));

        // Simulate sub-second processing
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(mock_signature)
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

    /// Force commits the current game state to Solana
    ///
    /// This can be used to checkpoint the game state during gameplay.
    pub fn force_commit(&self, payer: &Keypair) -> Result<String, MagicBlockError> {
        let game_pda = match self.delegated_game_pda {
            Some(pda) => pda,
            None => return Err(MagicBlockError::NotDelegated),
        };

        info!("Force committing game {} state to Solana", game_pda);

        // Create commit instruction
        let commit_ix = self.create_commit_instruction(game_pda, payer.pubkey())?;

        // Route to appropriate destination
        self.route_transaction(vec![commit_ix], payer)
    }

    /// Creates a commit instruction to checkpoint game state
    fn create_commit_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
        ];

        // Instruction data: [8-byte discriminator]
        let data = vec![0x03]; // Commit discriminator

        Ok(Instruction::new_with_bytes(
            self.config.program_id,
            &data,
            accounts,
        ))
    }
}

/// Plugin for Magic Block resolver integration
pub struct MagicBlockResolverPlugin;

impl Plugin for MagicBlockResolverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MagicBlockConfig>();
        app.init_resource::<MagicBlockResolver>();

        info!("Magic Block Resolver plugin initialized");
    }
}

/// Events for Magic Block resolver
#[derive(Event, Debug, Clone)]
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
    /// Transaction routed to Solana
    TransactionRoutedToSolana { signature: String },
    /// Force commit completed
    ForceCommitCompleted { game_pda: Pubkey, signature: String },
}

/// System to handle Magic Block events
pub fn handle_magic_block_events(mut events: EventReader<MagicBlockEvent>) {
    for event in events.read() {
        match event {
            MagicBlockEvent::GameDelegated { game_pda } => {
                info!("Game {} successfully delegated to ER", game_pda);
            }
            MagicBlockEvent::GameUndelegated { game_pda } => {
                info!("Game {} successfully undelegated from ER", game_pda);
            }
            MagicBlockEvent::DelegationFailed { game_pda, error } => {
                error!("Failed to delegate game {}: {}", game_pda, error);
            }
            MagicBlockEvent::UndelegationFailed { game_pda, error } => {
                error!("Failed to undelegate game {}: {}", game_pda, error);
            }
            MagicBlockEvent::TransactionRoutedToEr { signature } => {
                info!("Transaction routed to ER: {}", signature);
            }
            MagicBlockEvent::TransactionRoutedToSolana { signature } => {
                info!("Transaction routed to Solana: {}", signature);
            }
            MagicBlockEvent::ForceCommitCompleted {
                game_pda,
                signature,
            } => {
                info!(
                    "Force commit completed for game {}: {}",
                    game_pda, signature
                );
            }
        }
    }
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
