pub mod accounts;
mod budget;
pub mod flows;
pub mod instructions;
pub mod moves;
pub mod recovery;
pub mod rpc;
pub mod wallets;

pub use accounts::{fetch_profile_elo, fetch_tournament_places, TOURNAMENT_STATUS_COMPLETED};
pub use budget::apply_compute_budget;
pub use rpc::{base_client, er_client, get_blockhash_for_accounts, with_retry};

pub const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

pub const BASE_RPC_URL: &str = "https://api.devnet.solana.com";

pub const ER_RPC_URL: &str = "https://devnet-router.magicblock.app";

/// XFChess reimbursement accounting, independent of MagicBlock deployment pricing.
pub const CONTRACT_ER_SESSION_REIMBURSEMENT: u64 = 300_000;

pub const MASTER_MIN_BALANCE: u64 = 50_000_000;

pub const CHILD_FUNDING_AMOUNT: u64 = 50_000_000;

pub const MASTER_KEYPAIR_PATH: &str = "keys/program-authority.json";

pub const CHILDREN_KEYPAIR_PATH: &str = "keys/er-cu-children.json";

pub const RPC_RETRY_COUNT: u32 = 5;

pub const RPC_RETRY_DELAY_MS: u64 = 2_000;

pub fn unique_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
