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
    /// Magic Router RPC URL — MagicBlock's generic per-transaction router
    /// (default: devnet router endpoint). Used for ER writes (record_move,
    /// undelegate); base writes use `solana_rpc_url`.
    pub magic_router_rpc_url: String,
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
    /// Base58 encoded external-elo linking authority private key
    pub link_authority_key: Option<String>,
    /// Base58 encoded treasury-withdrawal authority private key. Signs
    /// `withdraw_treasury` (platform-fee payouts / manual refunds). Must
    /// correspond to `treasury_authority::ID` in the on-chain program.
    pub treasury_authority_key: Option<String>,
    /// Admin token for protected endpoints (POST /admin/dispute/resolve, etc.)
    pub admin_token: Option<String>,
    /// Tournament entry-fee recipient pubkey — a *different* address from
    /// `treasury_authority_key` above (that one is the withdraw-authority
    /// signer; this one just receives entry fees directly). Deliberately
    /// distinctly named to avoid the two being conflated — see
    /// docs/plans/identity-implementation-plan.md.
    pub tournament_fee_recipient: String,
    /// USDC mint address (devnet or mainnet)
    pub usdc_mint_pubkey: String,
    /// Lichess OAuth client ID (from lichess.org/account/oauth/app)
    pub lichess_client_id: String,
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
    /// - `LINK_AUTHORITY_KEY` - Base58 external-elo linking authority key
    /// - `TOURNAMENT_FEE_RECIPIENT` - Host treasury pubkey for entry fees
    /// - `USDC_MINT` - USDC mint address (devnet or mainnet)
    /// - `LICHESS_CLIENT_ID` - Lichess OAuth client ID
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
            magic_router_rpc_url: env::var("MAGIC_ROUTER_RPC_URL")
                .or_else(|_| env::var("MAGIC_ROUTER_URL"))
                .unwrap_or_else(|_| "https://devnet-router.magicblock.app".into()),
            // Canonical program ID — matches `declare_id!` in programs/xfchess-game
            // and the deployed devnet program. Override with PROGRAM_ID for other
            // clusters/deployments.
            program_id: env::var("PROGRAM_ID")
                .unwrap_or_else(|_| "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into()),
            jwt_secret: env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set — generate with: openssl rand -hex 32"),
            identity_encryption_key: env::var("IDENTITY_ENCRYPTION_KEY").expect(
                "IDENTITY_ENCRYPTION_KEY must be set — generate with: openssl rand -hex 32",
            ),
            identity_salt: env::var("IDENTITY_SALT")
                .expect("IDENTITY_SALT must be set — generate with: openssl rand -hex 32"),
            fee_payer_keys: env::var("FEE_PAYER_KEYS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            vps_authority_key: env::var("VPS_AUTHORITY_KEY").ok(),
            kyc_authority_key: env::var("KYC_AUTHORITY_KEY").ok(),
            link_authority_key: env::var("LINK_AUTHORITY_KEY").ok(),
            treasury_authority_key: env::var("TREASURY_AUTHORITY_KEY").ok(),
            admin_token: env::var("ADMIN_TOKEN").ok(),
            tournament_fee_recipient: env::var("TOURNAMENT_FEE_RECIPIENT")
                .unwrap_or_else(|_| "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".to_string()),
            usdc_mint_pubkey: env::var("USDC_MINT")
                .unwrap_or_else(|_| "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".to_string()),
            lichess_client_id: env::var("LICHESS_CLIENT_ID").unwrap_or_default(),
        }
    }

    /// Validate config at startup so bad/placeholder secrets fail loudly instead of
    /// silently running an insecure production server.
    ///
    /// Behaviour depends on `APP_ENV`:
    /// - `APP_ENV=production` → any problem is a hard **error** (caller should exit).
    /// - otherwise (dev/local) → problems are **warnings** so `just backend` still runs
    ///   with the throwaway placeholders from the justfile.
    pub fn validate(&self) -> Result<(), String> {
        let prod = env::var("APP_ENV")
            .map(|v| v.eq_ignore_ascii_case("production"))
            .unwrap_or(false);

        let zero64 = "0".repeat(64);
        let one64 = "1".repeat(64);
        let is_hex64 = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());

        let checks: Vec<(bool, &str)> = vec![
            (
                self.jwt_secret.len() >= 32,
                "JWT_SECRET too short (need >= 32 chars; openssl rand -hex 32)",
            ),
            (
                self.jwt_secret != zero64,
                "JWT_SECRET is the all-zeros dev placeholder — generate a real one",
            ),
            (
                is_hex64(&self.identity_encryption_key),
                "IDENTITY_ENCRYPTION_KEY must be 64 hex chars (openssl rand -hex 32)",
            ),
            (
                self.identity_encryption_key != zero64,
                "IDENTITY_ENCRYPTION_KEY is the all-zeros dev placeholder",
            ),
            (
                is_hex64(&self.identity_salt),
                "IDENTITY_SALT must be 64 hex chars (openssl rand -hex 32)",
            ),
            (
                self.identity_salt != one64,
                "IDENTITY_SALT is the all-ones dev placeholder",
            ),
            (
                self.solana_rpc_url.starts_with("http"),
                "SOLANA_RPC_URL must be an http(s) URL",
            ),
        ];

        let mut problems: Vec<&str> = checks
            .into_iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, m)| m)
            .collect();
        if prod && self.fee_payer_keys.is_empty() {
            problems
                .push("FEE_PAYER_KEYS empty — backend cannot pay transaction fees in production");
        }

        if problems.is_empty() {
            return Ok(());
        }
        if prod {
            Err(problems
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n"))
        } else {
            for p in &problems {
                tracing::warn!("[config] {} (APP_ENV != production, continuing)", p);
            }
            Ok(())
        }
    }
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
