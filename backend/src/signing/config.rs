use std::env;

#[derive(Clone)]
pub struct SigningConfig {
    pub port: u16,
    pub solana_rpc_url: String,
    pub er_rpc_url: String,
    pub program_id: String,
    pub jwt_secret: String,
    /// Comma-separated base58 fee-payer private keys.
    pub fee_payer_keys: Vec<String>,
}

impl SigningConfig {
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
                .unwrap_or_else(|_| "change-me-in-production-32-bytes!!".into()),
            fee_payer_keys: env::var("FEE_PAYER_KEYS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }
}
