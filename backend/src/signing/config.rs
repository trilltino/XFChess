//! Configuration module for the XFChess signing service.
//!
//! This module loads configuration from environment variables for RPC endpoints,
//! program IDs, JWT secrets, and fee-payer keys.

use std::env;

/// Configuration structure for the signing service.
///
/// All fields are loaded from environment variables with sensible defaults
/// for development environments.
#[derive(Clone)]
pub struct SigningConfig {
    /// Port number for the HTTP server (default: 8090)
    pub port: u16,
    /// Solana RPC URL for mainnet/devnet (default: devnet)
    pub solana_rpc_url: String,
    /// MagicBlock Execution Rollup RPC URL (default: devnet EU endpoint)
    pub er_rpc_url: String,
    /// XFChess program ID on Solana
    pub program_id: String,
    /// Secret key for JWT token signing
    pub jwt_secret: String,
    /// 32-byte hex string for identity encryption (AES-256-GCM)
    pub identity_encryption_key: String,
    /// 32-byte hex string for identity blind index salt
    pub identity_salt: String,
    /// Comma-separated base58 fee-payer private keys for transaction fees
    pub fee_payer_keys: Vec<String>,
    /// Base58 encoded VPS authority private key
    pub vps_authority_key: Option<String>,
    /// Base58 encoded KYC authority private key
    pub kyc_authority_key: Option<String>,
    /// Host treasury pubkey - receives entry fees directly
    pub host_treasury_pubkey: String,
    /// USDC mint address (devnet or mainnet)
    pub usdc_mint_pubkey: String,
}

impl SigningConfig {
    /// Loads configuration from environment variables.
    ///
    /// # Environment Variables
    /// - `SIGNING_PORT` - Server port (default: 8090)
    /// - `SOLANA_RPC_URL` - Solana RPC endpoint (default: devnet)
    /// - `ER_RPC_URL` - MagicBlock ER endpoint (default: devnet EU)
    /// - `PROGRAM_ID` - XFChess program ID
    /// - `JWT_SECRET` - JWT signing secret
    /// - `IDENTITY_ENCRYPTION_KEY` - 64-char hex for AES-256
    /// - `IDENTITY_SALT` - 64-char hex for blind index
    /// - `FEE_PAYER_KEYS` - Comma-separated base58 keys or file paths
    /// - `VPS_AUTHORITY_KEY` - Base58 VPS authority key
    /// - `KYC_AUTHORITY_KEY` - Base58 KYC authority key
    /// - `HOST_TREASURY_PUBKEY` - Host treasury pubkey for entry fees
    /// - `USDC_MINT` - USDC mint address (devnet or mainnet)
    ///
    /// # Returns
    /// A fully configured `SigningConfig` struct
    pub fn from_env() -> Self {
        Self {
            port: env::var("SIGNING_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8090),
            solana_rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.devnet.solana.com".into()),
            er_rpc_url: env::var("ER_RPC_URL")
                .unwrap_or_else(|_| "https://devnet-eu.magicblock.app/".into()),
            program_id: env::var("PROGRAM_ID")
                .unwrap_or_else(|_| "AhkTK5LVJHvR51gmDXbsJsqq4wg381AH6vTiaFGGJPWm".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "11111111111111111111111111111111".to_string()),
            identity_encryption_key: env::var("IDENTITY_ENCRYPTION_KEY")
                .unwrap_or_else(|_| "0000000000000000000000000000000000000000000000000000000000000000".to_string()),
            identity_salt: env::var("IDENTITY_SALT")
                .unwrap_or_else(|_| "1111111111111111111111111111111111111111111111111111111111111111".to_string()),
            fee_payer_keys: env::var("FEE_PAYER_KEYS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            vps_authority_key: env::var("VPS_AUTHORITY_KEY").ok(),
            kyc_authority_key: env::var("KYC_AUTHORITY_KEY").ok(),
            host_treasury_pubkey: env::var("HOST_TREASURY_PUBKEY")
                .unwrap_or_else(|_| "11111111111111111111111111111111".to_string()),
            usdc_mint_pubkey: env::var("USDC_MINT")
                .unwrap_or_else(|_| "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".to_string()),
        }
    }
}
