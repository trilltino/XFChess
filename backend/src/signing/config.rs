//! Configuration module for the XFChess signing service.
//!
//! This module loads configuration from environment variables for RPC endpoints,
//! program IDs, JWT secrets, and fee-payer keys.

use solana_sdk::pubkey::Pubkey;
use std::env;
use std::str::FromStr;

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
    /// Optional dedicated mainnet RPC URL, for reads that are independent of the
    /// game program (which is devnet-only): price feeds, USDC balance checks for
    /// the fiat onramp, ad hoc lookups. `None` when unset — callers should fall
    /// back to a public mainnet endpoint or skip the mainnet-only feature.
    pub solana_mainnet_rpc_url: Option<String>,
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
    /// Public key of the treasury-withdrawal authority — NOT a secret; the
    /// private key is deliberately never loaded by this process. Must match
    /// `treasury_authority::ID` in the on-chain program. `withdraw_treasury`
    /// signing happens exclusively via the standalone `treasury_signer`
    /// binary, run by an operator on a separate, minimally-networked host —
    /// see that binary's module doc for why. Defaults to the deployed
    /// devnet/mainnet treasury authority's pubkey; override with
    /// `TREASURY_AUTHORITY_PUBKEY` only for a different deployment.
    pub treasury_authority_pubkey: String,
    /// Admin token for protected endpoints (POST /admin/dispute/resolve, etc.)
    pub admin_token: Option<String>,
    /// Tournament entry-fee recipient pubkey — a *different* address from
    /// `treasury_authority_pubkey` above (that one is the withdraw-authority
    /// signer; this one just receives entry fees directly). Deliberately
    /// distinctly named to avoid the two being conflated — see
    /// docs/plans/identity-implementation-plan.md.
    pub tournament_fee_recipient: String,
    /// USDC mint address (devnet or mainnet)
    pub usdc_mint_pubkey: String,
    /// Lichess OAuth client ID (from lichess.org/account/oauth/app)
    pub lichess_client_id: String,
    /// Comma-separated list of allowed CORS origins (e.g.
    /// `https://xfchess.com,https://www.xfchess.com`). Empty outside
    /// production means "allow any origin" (dev convenience) — see
    /// `infrastructure::router::cors_layer`. Must be non-empty in production;
    /// enforced by [`Self::validate`].
    pub allowed_origins: Vec<String>,
}

impl SigningConfig {
    /// True when `solana_rpc_url` points at a devnet cluster. Used to bypass
    /// the profile/KYC/CACF wager-eligibility gate for test SOL — mainnet
    /// stays fully gated regardless of this flag.
    pub fn is_devnet(&self) -> bool {
        self.solana_rpc_url.contains("devnet")
    }

    /// True when `APP_ENV=production`. Gates whether missing/invalid secrets
    /// (authority keys, JWT secret, CORS origins, ...) are a hard startup
    /// failure or a warn-and-continue dev convenience — see [`Self::validate`].
    pub fn is_production(&self) -> bool {
        env::var("APP_ENV")
            .map(|v| v.eq_ignore_ascii_case("production"))
            .unwrap_or(false)
    }

    /// Loads configuration from environment variables.
    ///
    /// # Environment Variables
    /// - `SIGNING_PORT` - Server port (default: 8090)
    /// - `SOLANA_RPC_URL` - Solana RPC endpoint (default: devnet)
    /// - `SOLANA_MAINNET_RPC_URL` - Optional dedicated mainnet RPC endpoint (default: unset)
    /// - `ER_RPC_URL` - MagicBlock ER endpoint (default: devnet EU)
    /// - `PROGRAM_ID` - XFChess program ID
    /// - `JWT_SECRET` - JWT signing secret
    /// - `IDENTITY_ENCRYPTION_KEY` - 64-char hex for AES-256
    /// - `IDENTITY_SALT` - 64-char hex for blind index
    /// - `FEE_PAYER_KEYS` - Comma-separated base58 keys or file paths
    /// - `VPS_AUTHORITY_KEY` - Base58 VPS authority key
    /// - `KYC_AUTHORITY_KEY` - Base58 KYC authority key
    /// - `LINK_AUTHORITY_KEY` - Base58 external-elo linking authority key
    /// - `TREASURY_AUTHORITY_PUBKEY` - Public key only (never the secret —
    ///   see `treasury_signer` bin); defaults to the deployed treasury pubkey
    /// - `TOURNAMENT_FEE_RECIPIENT` - Host treasury pubkey for entry fees
    /// - `USDC_MINT` - USDC mint address (devnet or mainnet)
    /// - `LICHESS_CLIENT_ID` - Lichess OAuth client ID
    /// - `ALLOWED_ORIGINS` - Comma-separated CORS allow-list (required in production)
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
            solana_mainnet_rpc_url: env::var("SOLANA_MAINNET_RPC_URL").ok(),
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
            treasury_authority_pubkey: env::var("TREASURY_AUTHORITY_PUBKEY")
                .unwrap_or_else(|_| "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string()),
            admin_token: env::var("ADMIN_TOKEN").ok(),
            tournament_fee_recipient: env::var("TOURNAMENT_FEE_RECIPIENT")
                .unwrap_or_else(|_| "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".to_string()),
            usdc_mint_pubkey: env::var("USDC_MINT")
                .unwrap_or_else(|_| "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".to_string()),
            lichess_client_id: env::var("LICHESS_CLIENT_ID").unwrap_or_default(),
            allowed_origins: env::var("ALLOWED_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
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
        let prod = self.is_production();

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
        if prod {
            match env::var("ADMIN_API_KEY") {
                Ok(key) if key.len() >= 32 => {}
                Ok(_) => problems.push("ADMIN_API_KEY too short (need >= 32 characters)"),
                Err(env::VarError::NotPresent) => {
                    problems.push("ADMIN_API_KEY not set — refusing to start without admin auth")
                }
                Err(env::VarError::NotUnicode(_)) => {
                    problems.push("ADMIN_API_KEY contains invalid UTF-8")
                }
            }
        }
        if prod && self.vps_authority_key.is_none() {
            problems.push(
                "VPS_AUTHORITY_KEY not set — refusing to generate a random fallback in production",
            );
        }
        if prod && self.kyc_authority_key.is_none() {
            problems.push(
                "KYC_AUTHORITY_KEY not set — refusing to generate a random fallback in production",
            );
        }
        if prod && self.link_authority_key.is_none() {
            problems.push(
                "LINK_AUTHORITY_KEY not set — refusing to generate a random fallback in production",
            );
        }
        if Pubkey::from_str(&self.treasury_authority_pubkey).is_err() {
            problems.push("TREASURY_AUTHORITY_PUBKEY is not a valid base58 pubkey");
        }
        if prod && self.allowed_origins.is_empty() {
            problems
                .push("ALLOWED_ORIGINS empty — refusing to allow any origin (CORS) in production");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialises the tests below — they all mutate the process-global
    /// `APP_ENV` var, same reasoning as `RELAY_TEST_LOCK` in `tests/e2e_api.rs`.
    static APP_ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn base_config() -> SigningConfig {
        SigningConfig {
            port: 8090,
            solana_rpc_url: "https://api.devnet.solana.com".into(),
            solana_mainnet_rpc_url: None,
            er_rpc_url: "https://devnet-eu.magicblock.app/".into(),
            magic_router_rpc_url: "https://devnet-router.magicblock.app".into(),
            program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
            jwt_secret: "a".repeat(32),
            identity_encryption_key: "a".repeat(64),
            identity_salt: "b".repeat(64),
            fee_payer_keys: vec!["dummy".into()],
            vps_authority_key: Some("dummy".into()),
            kyc_authority_key: Some("dummy".into()),
            link_authority_key: Some("dummy".into()),
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".into(),
            admin_token: None,
            tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
            usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
            lichess_client_id: String::new(),
            allowed_origins: vec!["https://xfchess.com".into()],
        }
    }

    #[test]
    fn missing_authority_key_is_fine_outside_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::remove_var("APP_ENV");
        let mut config = base_config();
        config.vps_authority_key = None;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn missing_authority_key_is_fatal_in_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        std::env::set_var("ADMIN_API_KEY", "a".repeat(32));
        let mut config = base_config();
        config.vps_authority_key = None;
        let result = config.validate();
        std::env::remove_var("APP_ENV");
        std::env::remove_var("ADMIN_API_KEY");
        let err = result.expect_err("missing VPS_AUTHORITY_KEY must be fatal in production");
        assert!(err.contains("VPS_AUTHORITY_KEY"));
    }

    #[test]
    fn all_authority_keys_present_passes_in_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        std::env::set_var("ADMIN_API_KEY", "a".repeat(32));
        let result = base_config().validate();
        std::env::remove_var("APP_ENV");
        std::env::remove_var("ADMIN_API_KEY");
        assert!(result.is_ok());
    }

    #[test]
    fn missing_admin_api_key_is_fatal_in_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        std::env::remove_var("ADMIN_API_KEY");
        let result = base_config().validate();
        std::env::remove_var("APP_ENV");
        let err = result.expect_err("missing ADMIN_API_KEY must be fatal in production");
        assert!(err.contains("ADMIN_API_KEY"));
    }

    #[test]
    fn invalid_treasury_pubkey_is_fatal_in_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        std::env::set_var("ADMIN_API_KEY", "a".repeat(32));
        let mut config = base_config();
        config.treasury_authority_pubkey = "not-a-real-pubkey".into();
        let result = config.validate();
        std::env::remove_var("APP_ENV");
        std::env::remove_var("ADMIN_API_KEY");
        let err = result.expect_err("an invalid TREASURY_AUTHORITY_PUBKEY must fail validate()");
        assert!(err.contains("TREASURY_AUTHORITY_PUBKEY"));
    }

    #[test]
    fn missing_allowed_origins_is_fatal_in_production() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        std::env::set_var("ADMIN_API_KEY", "a".repeat(32));
        let mut config = base_config();
        config.allowed_origins = vec![];
        let result = config.validate();
        std::env::remove_var("APP_ENV");
        std::env::remove_var("ADMIN_API_KEY");
        let err = result.expect_err("empty ALLOWED_ORIGINS must be fatal in production");
        assert!(err.contains("ALLOWED_ORIGINS"));
    }

    #[test]
    fn is_production_reflects_app_env() {
        let _guard = APP_ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("APP_ENV", "production");
        assert!(base_config().is_production());
        std::env::set_var("APP_ENV", "development");
        assert!(!base_config().is_production());
        std::env::remove_var("APP_ENV");
        assert!(!base_config().is_production());
    }
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
