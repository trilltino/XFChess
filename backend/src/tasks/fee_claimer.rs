//! Fee claimer service for the XFChess backend.
//!
//! This service periodically checks the platform fee vault and claims
/// accumulated fees when the threshold is reached.

use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{info, debug, warn};

/// Runs the fee claimer service.
///
/// This service checks the PlatformFeeVault hourly and claims fees
/// when the vault accumulates more than 0.05 SOL.
///
/// # Arguments
/// * `rpc_url` - The Solana RPC URL
/// * `program_id_str` - The XFChess program ID as a string
pub async fn run_fee_claimer_service(rpc_url: String, program_id_str: String) {
    // Hourly check
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
    tokio::time::sleep(std::time::Duration::from_secs(10)).await; // test once shortly after boot
    
    let rpc_client = RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    );
    let program_id = Pubkey::from_str(&program_id_str).unwrap_or_default();
    
    loop {
        info!("[FeeClaimer] Checking PlatformFeeVault threshold...");
        let (vault_pda, _) = Pubkey::find_program_address(&[b"platform_vault"], &program_id);
        
        match rpc_client.get_account(&vault_pda) {
            Ok(acct) => {
                // Logic to check threshold (Example: claims if vault accumulated > 0.05 SOL)
                if acct.lamports > 50_000_000 {
                    info!("[FeeClaimer] Vault has {} lamports. Time to dispatch claim_fees!", acct.lamports);
                    // TODO: Map feepayer_pool to sign a claim instruction!
                } else {
                    debug!("[FeeClaimer] Vault only has {}, returning.", acct.lamports);
                }
            }
            Err(e) => {
                warn!("[FeeClaimer] Cannot fetch PlatformFeeVault: {}", e);
            }
        }
        interval.tick().await;
    }
}
