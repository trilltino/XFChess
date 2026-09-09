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

pub const XFCHESS_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

pub const MAGIC_BLOCK_ER_ENDPOINT: &str = "https://devnet-router.magicblock.app";

pub const MAGIC_BLOCK_EXPLORER: &str = "https://explorer.solana.com";

pub const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";

pub fn er_explorer_url_for(er_endpoint: &str, signature: &str) -> String {
    let endpoint = er_endpoint.trim_end_matches('/');
    format!(
        "{}/tx/{}?cluster=custom&customUrl={}",
        MAGIC_BLOCK_EXPLORER, signature, endpoint,
    )
}

fn anchor_disc(fn_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", fn_name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

#[derive(Error, Debug, Clone)]
pub enum MagicBlockError {
    #[error("Failed to delegate game PDA: {0}")]
    DelegationFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelegationStatus {
    #[default]
    Undelegated,
    Delegated,
}

#[derive(Resource, Clone, Debug)]
pub struct MagicBlockConfig {
    pub er_endpoint: String,
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

#[derive(Resource)]
pub struct MagicBlockResolver {
    config: MagicBlockConfig,
    pub delegation_status: DelegationStatus,
    pub delegated_game_pda: Option<Pubkey>,
    delegated_game_id: Option<u64>,
    pub solana_rpc: Option<Arc<solana_client::rpc_client::RpcClient>>,
}

impl Default for MagicBlockResolver {
    fn default() -> Self {
        Self::new(MagicBlockConfig::default())
    }
}

impl MagicBlockResolver {
    pub fn new(config: MagicBlockConfig) -> Self {
        Self {
            config,
            delegation_status: DelegationStatus::Undelegated,
            delegated_game_pda: None,
            delegated_game_id: None,
            solana_rpc: None,
        }
    }

    pub fn set_solana_rpc(&mut self, rpc_client: Arc<solana_client::rpc_client::RpcClient>) {
        self.solana_rpc = Some(rpc_client);
    }

    pub fn is_delegated(&self) -> bool {
        self.delegation_status == DelegationStatus::Delegated && self.delegated_game_pda.is_some()
    }

    pub fn get_delegated_game(&self) -> Option<Pubkey> {
        self.delegated_game_pda
    }

    pub fn set_game_id(&mut self, game_id: u64) {
        self.delegated_game_id = Some(game_id);
    }

    pub fn er_explorer_url(&self, signature: &str) -> String {
        er_explorer_url_for(&self.config.er_endpoint, signature)
    }

    pub fn er_endpoint(&self) -> &str {
        &self.config.er_endpoint
    }

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

#[derive(Event, Message, Debug, Clone)]
pub enum MagicBlockEvent {
    GameDelegated { game_pda: Pubkey },
    GameUndelegated { game_pda: Pubkey },
    DelegationFailed { game_pda: Pubkey, error: String },
    UndelegationFailed { game_pda: Pubkey, error: String },
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
