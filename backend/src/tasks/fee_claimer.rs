//! Fee claimer service for the XFChess backend.
//!
//! This service periodically checks the platform fee vault and claims
use crate::signing::{solana, FeepayerPool};
/// accumulated fees when the threshold is reached.
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Fee claimer check interval (seconds) - hourly checks
pub const FEE_CLAIMER_INTERVAL_SECONDS: u64 = 3600;

/// Initial test delay after boot (seconds)
pub const FEE_CLAIMER_INITIAL_DELAY_SECONDS: u64 = 10;

/// Threshold for claiming fees from vault (lamports) - 0.05 SOL
pub const FEE_CLAIM_THRESHOLD_LAMPORTS: u64 = 50_000_000;

/// Runs the fee claimer service.
///
/// This service checks the PlatformFeeVault hourly and claims fees
/// when the vault accumulates more than 0.05 SOL.
///
/// # Arguments
/// * `rpc_url` - The Solana RPC URL
/// * `program_id_str` - The XFChess program ID as a string
/// * `feepayer` - The fee-payer keypair pool for signing transactions
pub async fn run_fee_claimer_service(
    rpc_url: String,
    program_id_str: String,
    feepayer: Arc<FeepayerPool>,
) {
    // Hourly check
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(FEE_CLAIMER_INTERVAL_SECONDS));
    tokio::time::sleep(std::time::Duration::from_secs(
        FEE_CLAIMER_INITIAL_DELAY_SECONDS,
    ))
    .await; // test once shortly after boot

    let rpc_client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let program_id = match Pubkey::from_str(&program_id_str) {
        Ok(program_id) => program_id,
        Err(e) => {
            warn!("[FeeClaimer] Invalid program_id in configuration: {}", e);
            return;
        }
    };

    loop {
        info!("[FeeClaimer] Checking PlatformFeeVault threshold...");
        let (vault_pda, _) =
            Pubkey::find_program_address(&[solana::PLATFORM_FEE_VAULT_SEED], &program_id);

        match rpc_client.get_account(&vault_pda) {
            Ok(acct) => {
                // Check threshold: claims if vault accumulated > 0.05 SOL
                if acct.lamports > FEE_CLAIM_THRESHOLD_LAMPORTS {
                    info!(
                        "[FeeClaimer] Vault has {} lamports. Attempting to claim fees...",
                        acct.lamports
                    );

                    // Deserialize vault to get host_wallet
                    // Note: This is a simplified deserialization. In production,
                    // use the actual PlatformFeeVault struct from the program.
                    // Offset for host_wallet is after the discriminator (8 bytes)
                    let host_wallet = if acct.data.len() >= 40 {
                        let mut bytes = [0u8; 32];
                        bytes.copy_from_slice(&acct.data[8..40]);
                        Pubkey::from(bytes)
                    } else {
                        warn!("[FeeClaimer] Vault data too small to extract host_wallet");
                        interval.tick().await;
                        continue;
                    };

                    // Build claim_fees instruction
                    let fee_payer = feepayer.next();
                    let claim_ix =
                        solana::claim_fees_ix(&program_id, &fee_payer.pubkey(), &host_wallet);

                    // Sign and submit transaction
                    match solana::sign_and_submit(&rpc_client, fee_payer, &[claim_ix]) {
                        Ok(sig) => {
                            info!(
                                "[FeeClaimer] Successfully claimed fees with signature: {}",
                                sig
                            );
                        }
                        Err(e) => {
                            warn!("[FeeClaimer] Failed to claim fees: {}", e);
                        }
                    }
                } else {
                    debug!(
                        "[FeeClaimer] Vault only has {} lamports, below threshold.",
                        acct.lamports
                    );
                }
            }
            Err(e) => {
                warn!("[FeeClaimer] Cannot fetch PlatformFeeVault: {}", e);
            }
        }
        interval.tick().await;
    }
}
