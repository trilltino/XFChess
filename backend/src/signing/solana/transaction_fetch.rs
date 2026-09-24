use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use solana_client::{rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::{TransactionVersion, VersionedTransaction},
};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiLoadedAddresses, UiTransactionEncoding,
};

pub const MAX_SUPPORTED_TX_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionBuildPolicy {
    Legacy,
    V0,
    V1,
    Auto,
}

impl Default for TransactionBuildPolicy {
    fn default() -> Self {
        Self::Auto
    }
}

impl TransactionBuildPolicy {
    pub fn v1_send_enabled() -> bool {
        std::env::var("XFCHESS_ENABLE_TX_V1_SEND")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    pub fn choose_for_size(
        estimated_bytes: usize,
        wallet_supports_v0: bool,
        wallet_supports_v1: bool,
    ) -> Self {
        const LEGACY_SOFT_LIMIT: usize = 1232;
        const V1_SOFT_LIMIT: usize = 4096;

        if estimated_bytes <= LEGACY_SOFT_LIMIT {
            Self::Legacy
        } else if estimated_bytes <= V1_SOFT_LIMIT && wallet_supports_v1 && Self::v1_send_enabled()
        {
            Self::V1
        } else if wallet_supports_v0 {
            Self::V0
        } else {
            Self::Auto
        }
    }
}

#[derive(Debug)]
pub struct FetchedSolanaTransaction {
    pub signature: Signature,
    pub confirmed: EncodedConfirmedTransactionWithStatusMeta,
    pub decoded: VersionedTransaction,
    pub version: String,
    pub static_account_keys: Vec<Pubkey>,
    pub loaded_writable_accounts: Vec<String>,
    pub loaded_readonly_accounts: Vec<String>,
    pub transaction_size_bytes: Option<usize>,
    pub fetch_warnings: Vec<String>,
    pub rpc_v1_fetch_supported: bool,
}

pub fn v1_read_required() -> bool {
    std::env::var("XFCHESS_TX_V1_READ_REQUIRED")
        .map(|v| v == "1")
        .unwrap_or(false)
}

pub fn transaction_fetch_config() -> RpcTransactionConfig {
    RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Base64),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(MAX_SUPPORTED_TX_VERSION),
    }
}

pub fn fetch_transaction_v1_aware(
    rpc: &RpcClient,
    signature: &Signature,
) -> anyhow::Result<FetchedSolanaTransaction> {
    let confirmed = rpc
        .get_transaction_with_config(signature, transaction_fetch_config())
        .map_err(|err| {
            let message = err.to_string();
            if looks_like_tx_version_support_error(&message) {
                anyhow!(
                    "RPC does not appear to support Solana transaction v1 reads \
                     with maxSupportedTransactionVersion={}: {}",
                    MAX_SUPPORTED_TX_VERSION,
                    message
                )
            } else {
                anyhow!(message)
            }
        })
        .with_context(|| format!("transaction fetch failed for {signature}"))?;

    let decoded = confirmed
        .transaction
        .transaction
        .decode()
        .ok_or_else(|| anyhow!("transaction could not be decoded"))?;

    let version = transaction_version_label(decoded.version());
    let static_account_keys = decoded.message.static_account_keys().to_vec();
    let transaction_size_bytes = bincode::serialize(&decoded).ok().map(|bytes| bytes.len());
    let (loaded_writable_accounts, loaded_readonly_accounts) = loaded_address_lists(&confirmed);

    Ok(FetchedSolanaTransaction {
        signature: *signature,
        confirmed,
        decoded,
        version,
        static_account_keys,
        loaded_writable_accounts,
        loaded_readonly_accounts,
        transaction_size_bytes,
        fetch_warnings: Vec::new(),
        rpc_v1_fetch_supported: true,
    })
}

fn loaded_address_lists(
    confirmed: &EncodedConfirmedTransactionWithStatusMeta,
) -> (Vec<String>, Vec<String>) {
    let Some(meta) = confirmed.transaction.meta.as_ref() else {
        return (Vec::new(), Vec::new());
    };
    let loaded = Option::<UiLoadedAddresses>::from(meta.loaded_addresses.clone());
    loaded
        .map(|addresses| (addresses.writable, addresses.readonly))
        .unwrap_or_default()
}

fn transaction_version_label(version: TransactionVersion) -> String {
    match version {
        TransactionVersion::LEGACY => "legacy".to_string(),
        TransactionVersion::Number(n) => format!("v{n}"),
    }
}

fn looks_like_tx_version_support_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("maxsupportedtransactionversion")
        || lower.contains("max supported transaction version")
        || lower.contains("unsupported transaction version")
        || lower.contains("transaction version")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_fetch_config_requests_v1() {
        let cfg = transaction_fetch_config();
        assert_eq!(
            cfg.max_supported_transaction_version,
            Some(MAX_SUPPORTED_TX_VERSION)
        );
        assert_eq!(cfg.encoding, Some(UiTransactionEncoding::Base64));
    }

    #[test]
    fn build_policy_keeps_simple_flows_legacy() {
        assert_eq!(
            TransactionBuildPolicy::choose_for_size(900, true, true),
            TransactionBuildPolicy::Legacy
        );
    }
}
