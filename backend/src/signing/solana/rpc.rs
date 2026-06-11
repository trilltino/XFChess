//! RPC client helpers for Solana.

use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

/// Creates an RPC client with confirmed commitment.
pub fn make_rpc(url: &str) -> RpcClient {
    RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed())
}

/// Returns the configured RPC URL from the environment, falling back to devnet.
/// Used by blinks code that doesn't have access to `AppState`.
pub fn rpc_url_or_devnet() -> String {
    std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
}
