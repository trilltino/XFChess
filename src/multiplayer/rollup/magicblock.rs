//! Magic Block Ephemeral Rollups Resolver Integration
//!
//! Builds the `delegate_game` instruction (signed by the wallet) and tracks
//! local delegation status for the UI. Actual move/undelegate RPC routing
//! between base RPC and MagicBlock's Magic Router is owned by the backend
//! (see `crate::multiplayer::vps_client` and MAGICBLOCK.md) — this module
//! does not send gameplay transactions itself.
//!
//! Reference: https://docs.magicblock.gg/

use bevy::prelude::*;
use sha2::{Digest, Sha256};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use std::sync::Arc;
use thiserror::Error;

#[cfg(feature = "solana")]
use ephemeral_rollups_sdk::pda::{
    delegate_buffer_pda_from_delegated_account_and_owner_program,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};

/// The XFChess program ID on Solana
pub const XFCHESS_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

/// Magic Block ER validator endpoint (used for the explorer link only)
pub const MAGIC_BLOCK_ER_ENDPOINT: &str = "https://devnet-eu.magicblock.app";

/// Solana Explorer with custom RPC for viewing ER transactions
pub const MAGIC_BLOCK_EXPLORER: &str = "https://explorer.solana.com";

/// MagicBlock Delegation Program ID
pub const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";

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
}

/// Represents the delegation status of a game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelegationStatus {
    #[default]
    Undelegated,
    Delegated,
}

/// Configuration for the Magic Block resolver
#[derive(Resource, Clone, Debug)]
pub struct MagicBlockConfig {
    /// The ER validator endpoint URL (used for the explorer link only —
    /// actual move/undelegate RPC routing is owned by the backend, see
    /// MAGICBLOCK.md).
    pub er_endpoint: String,
    /// The Solana program ID for XFChess
    pub program_id: Pubkey,
}

impl Default for MagicBlockConfig {
    fn default() -> Self {
        Self {
            er_endpoint: MAGIC_BLOCK_ER_ENDPOINT.to_string(),
            program_id: XFCHESS_PROGRAM_ID.parse().unwrap_or_default(),
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
    /// RPC client for Solana fallback (public for async task spawning)
    pub solana_rpc: Option<Arc<solana_client::rpc_client::RpcClient>>,
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
        let endpoint = self.config.er_endpoint.trim_end_matches('/');
        format!(
            "{}/tx/{}?cluster=custom&customUrl={}",
            MAGIC_BLOCK_EXPLORER, signature, endpoint,
        )
    }

    /// Creates a `delegate_game` Anchor instruction matching the on-chain
    /// `DelegateGameCtx` account layout.
    ///   0.  game                  (mut) — game PDA
    ///   1.  payer          (mut, sign)  — pays for delegation bookkeeping
    ///   2.  owner_program               — xfchess-game program itself
    ///   3.  buffer                (mut) — game delegation buffer PDA
    ///   4.  delegation_record     (mut) — game delegation record PDA
    ///   5.  delegation_metadata   (mut) — game delegation metadata PDA
    ///   6.  delegation_program          — MagicBlock delegation program
    ///   7.  system_program
    ///   8.  fee_payer       (mut, sign) — must equal `game.fee_payer`
    /// `payer` funds the MagicBlock delegation bookkeeping accounts (buffer,
    /// delegation_record, delegation_metadata) — it does not have to be the
    /// wallet, just some funded signer; `fee_payer` must equal `game.fee_payer`
    /// on-chain (checked by `handler_delegate_game`) — pass the session key
    /// there to co-sign without a second real "authority" check.
    ///
    /// Account order below matches `DelegateGameCtx`
    /// (`programs/xfchess-game/src/delegation_ix/delegate.rs`) exactly — Anchor
    /// resolves `Accounts` fields positionally, so previously this built a
    /// 12-account list (interleaving a `move_log` delegation that has no
    /// field in `DelegateGameCtx` at all) which put a non-signer PDA where
    /// `payer: Signer` was expected — every delegation submitted through it
    /// would have failed on-chain with `AccountNotSigner`, regardless of who
    /// signed. If move_log ever needs delegating too, that has to be a
    /// second, separate instruction — not folded into this account list.
    pub fn create_delegation_instruction(
        &self,
        game_pda: Pubkey,
        payer: Pubkey,
        fee_payer: Pubkey,
    ) -> Result<Instruction, MagicBlockError> {
        let delegation_program_id: Pubkey = DELEGATION_PROGRAM_ID
            .parse()
            .map_err(|_| MagicBlockError::DelegationFailed("Bad delegation program id".into()))?;

        let game_id = self.delegated_game_id.unwrap_or(0);

        let buffer_pda = {
            let pda = delegate_buffer_pda_from_delegated_account_and_owner_program(
                &game_pda.to_bytes().into(),
                &self.config.program_id.to_bytes().into(),
            );
            Pubkey::new_from_array(pda.to_bytes())
        };
        let delegation_record = {
            let pda = delegation_record_pda_from_delegated_account(&game_pda.to_bytes().into());
            Pubkey::new_from_array(pda.to_bytes())
        };
        let delegation_metadata = {
            let pda = delegation_metadata_pda_from_delegated_account(&game_pda.to_bytes().into());
            Pubkey::new_from_array(pda.to_bytes())
        };

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(game_pda, false),
            solana_sdk::instruction::AccountMeta::new(payer, true),
            solana_sdk::instruction::AccountMeta::new_readonly(self.config.program_id, false),
            solana_sdk::instruction::AccountMeta::new(buffer_pda, false),
            solana_sdk::instruction::AccountMeta::new(delegation_record, false),
            solana_sdk::instruction::AccountMeta::new(delegation_metadata, false),
            solana_sdk::instruction::AccountMeta::new_readonly(delegation_program_id, false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_system_interface::program::id(),
                false,
            ),
            solana_sdk::instruction::AccountMeta::new(fee_payer, true),
        ];

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

        resolver.delegation_status = DelegationStatus::Delegated;
        assert!(!resolver.is_delegated()); // still false: no delegated_game_pda set

        resolver.delegated_game_pda = Some(Pubkey::default());
        assert!(resolver.is_delegated());

        resolver.delegation_status = DelegationStatus::Undelegated;
        assert!(!resolver.is_delegated());
    }

    #[test]
    fn test_magic_block_config_default() {
        let config = MagicBlockConfig::default();

        assert_eq!(config.er_endpoint, MAGIC_BLOCK_ER_ENDPOINT);
    }

    #[test]
    fn test_magic_block_error_display() {
        let err = MagicBlockError::DelegationFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }
}
