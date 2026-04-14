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
                .unwrap_or_else(|_| "FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX".into()),
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
        }
    }
}
