//! ER CU Benchmark - XFChess Ephemeral Rollup Compute Unit Test Suite

pub mod cost_reporter;
pub mod cu_logger;
pub mod game_flows;
pub mod instructions;
pub mod keygen;
pub mod moves;
pub mod rpc_bench;

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Program ID for XFChess on devnet.
pub const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

/// Base-layer devnet RPC endpoint.
pub const BASE_RPC_URL: &str = "https://api.devnet.solana.com";

/// MagicBlock ephemeral rollup devnet endpoint.
pub const ER_RPC_URL: &str = "https://devnet-eu.magicblock.app";

/// Default compute-unit limit per transaction.
pub const DEFAULT_CU_LIMIT: u32 = 1_400_000;

/// Default compute-unit price in micro-lamports.
pub const DEFAULT_CU_PRICE: u64 = 10_000;

/// Default heap size in bytes.
pub const DEFAULT_HEAP_SIZE: u32 = 256_000;

/// Lamports per SOL.
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// SOL price in GBP for cost estimation.
pub const SOL_GBP_RATE: f64 = 60.0;

/// Transaction base fee in lamports.
pub const BASE_TX_FEE: u64 = 5_000;

/// ER priority fee in lamports.
pub const ER_PRIORITY_FEE: u64 = 10_000;

/// Minimum lamports to keep in master wallet (0.05 SOL buffer).
pub const MASTER_MIN_BALANCE: u64 = 50_000_000;

/// Default funding amount per child wallet (0.05 SOL).
pub const CHILD_FUNDING_AMOUNT: u64 = 50_000_000;

/// Master keypair file path — funded devnet wallet (1.5+ SOL).
pub const MASTER_KEYPAIR_PATH: &str = "keys/program-authority.json";

/// Child keypairs file path.
pub const CHILDREN_KEYPAIR_PATH: &str = "keys/er-cu-children.json";

/// Retry count for RPC calls.
pub const RPC_RETRY_COUNT: u32 = 5;

/// Delay between retries in milliseconds.
pub const RPC_RETRY_DELAY_MS: u64 = 2_000;

/// Create a base-layer RPC client.
pub fn base_client() -> RpcClient {
    RpcClient::new_with_commitment(BASE_RPC_URL.to_string(), CommitmentConfig::confirmed())
}

/// Create an ER-layer RPC client.
pub fn er_client() -> RpcClient {
    RpcClient::new_with_commitment(ER_RPC_URL.to_string(), CommitmentConfig::confirmed())
}

/// Parse a pubkey from a string.
pub fn parse_pubkey(s: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(s).map_err(|e| format!("Invalid pubkey: {}", e))
}

/// Build a compute budget CU limit instruction.
pub fn compute_budget_limit(cu_limit: u32) -> solana_sdk::instruction::Instruction {
    solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(cu_limit)
}

/// Build a compute budget CU price instruction.
pub fn compute_budget_price(micro_lamports: u64) -> solana_sdk::instruction::Instruction {
    solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(micro_lamports)
}

/// Build a compute budget heap frame instruction.
pub fn compute_budget_heap(heap_size: u32) -> solana_sdk::instruction::Instruction {
    solana_compute_budget_interface::ComputeBudgetInstruction::request_heap_frame(heap_size)
}

/// Apply compute budget optimizations to a transaction.
pub fn apply_compute_budget(
    ixs: &mut Vec<solana_sdk::instruction::Instruction>,
    cu_limit: u32,
    cu_price: u64,
    heap_size: u32,
) {
    ixs.insert(0, compute_budget_limit(cu_limit));
    ixs.insert(1, compute_budget_price(cu_price));
    ixs.insert(2, compute_budget_heap(heap_size));
}

/// Retry wrapper for RPC calls with exponential backoff.
pub async fn with_retry<F, T>(mut f: F) -> Result<T, anyhow::Error>
where
    F: FnMut() -> Result<T, solana_client::client_error::ClientError>,
{
    let mut delay = RPC_RETRY_DELAY_MS;
    for attempt in 0..RPC_RETRY_COUNT {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                eprintln!("   RPC attempt {} failed: {}", attempt + 1, e);
                if let solana_client::client_error::ClientErrorKind::RpcError(
                    solana_client::rpc_request::RpcError::RpcResponseError { data, .. },
                ) = &*e.kind
                {
                    eprintln!("   RPC ERROR DATA: {:?}", data);
                    use solana_client::rpc_request::RpcResponseErrorData;
                    match data {
                        RpcResponseErrorData::SendTransactionPreflightFailure(result) => {
                            if let Some(logs) = &result.logs {
                                eprintln!("   FULL SIMULATION LOGS ({} entries):", logs.len());
                                for log in logs {
                                    eprintln!("     {}", log);
                                }
                            }
                        }
                        _ => {
                            eprintln!("   OTHER ERROR DATA TYPE");
                        }
                    }
                }
                if attempt + 1 < RPC_RETRY_COUNT {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    delay *= 2;
                }
            }
        }
    }
    Err(anyhow::anyhow!("Max retries exceeded"))
}

/// Generate a unique ID based on timestamp.
pub fn unique_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Fetch a player's ELO rating from their on-chain profile.
/// Returns the ELO as f64 (scaled: 120000.0 = 1200 ELO).
pub fn fetch_profile_elo(
    rpc: &RpcClient,
    program_id: Pubkey,
    player: Pubkey,
) -> anyhow::Result<f64> {
    let profile_pda = Pubkey::find_program_address(&[b"profile", player.as_ref()], &program_id).0;
    let account = rpc.get_account_data(&profile_pda)?;
    if account.len() < 8 + 32 + 8 + 4 + 4 + 4 + 8 + 8 {
        return Err(anyhow::anyhow!("Profile account data too short"));
    }
    // Anchor discriminator (8 bytes) + authority (32) + created_at (8) + wins (4) + losses (4) + draws (4) + games_played (4) + elo_rating (8 f64)
    // ELO rating starts at offset 8 + 32 + 8 + 4 + 4 + 4 + 4 = 64
    let mut elo_bytes = [0u8; 8];
    elo_bytes.copy_from_slice(&account[64..72]);
    let elo = f64::from_le_bytes(elo_bytes);
    Ok(elo)
}
