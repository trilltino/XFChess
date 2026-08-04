//! ER CU Benchmark - XFChess Ephemeral Rollup Compute Unit Test Suite

pub mod cost_reporter;
pub mod cu_logger;
pub mod game_flows;
pub mod instructions;
pub mod keygen;
pub mod moves;
pub mod recovery_drill;
pub mod rpc_bench;

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Program ID for XFChess on devnet.
pub const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

/// Base-layer devnet RPC endpoint.
pub const BASE_RPC_URL: &str = "https://api.devnet.solana.com";

/// MagicBlock ephemeral rollup devnet endpoint — the router, not a raw
/// regional validator URL. A direct regional endpoint (e.g. `devnet-eu.
/// magicblock.app`) isn't guaranteed to be the specific ER instance a given
/// account was actually delegated to, which surfaces as "Transaction loads a
/// writable account that cannot be written" the moment you try to write to
/// it. The production backend never talks to a regional endpoint directly —
/// see `backend/src/signing/config.rs`'s `magic_router_rpc_url` (env
/// `MAGIC_ROUTER_RPC_URL`, this same default), used for every ER RPC call
/// (`backend/src/signing/routes/main.rs`, `tasks/settlement_worker.rs`).
pub const ER_RPC_URL: &str = "https://devnet-router.magicblock.app";

/// Fetch a blockhash valid for a transaction touching `accounts`, routed
/// correctly by the Magic Router.
///
/// Plain `getLatestBlockhash` against a router endpoint (as opposed to a
/// single validator) doesn't work for ER-delegated accounts: the router picks
/// which shard actually executes a transaction based on the *accounts it
/// touches*, but a blockhash fetched with no account context can come back
/// from an arbitrary/default shard — one that never advances the blockhash
/// the real target shard will check against, so the send later fails with
/// "Blockhash not found" on every retry, not just a transient one. The router
/// exposes a dedicated method for this exact problem:
/// <https://docs.magicblock.gg/pages/ephemeral-rollups-ers/api-reference/er/getBlockhashForAccounts>
pub fn get_blockhash_for_accounts(
    rpc: &RpcClient,
    accounts: &[Pubkey],
) -> solana_client::client_error::Result<solana_sdk::hash::Hash> {
    use solana_client::rpc_request::RpcRequest;

    #[derive(serde::Deserialize)]
    struct BlockhashForAccountsResponse {
        blockhash: String,
    }

    let addrs: Vec<String> = accounts.iter().map(|p| p.to_string()).collect();
    let params = serde_json::json!([addrs]);
    let resp: BlockhashForAccountsResponse = rpc.send(
        RpcRequest::Custom {
            method: "getBlockhashForAccounts",
        },
        params,
    )?;
    Ok(resp
        .blockhash
        .parse()
        .expect("router returned an invalid blockhash string"))
}

/// Sends `tx` and polls for confirmation with a short, tight interval instead
/// of relying on `RpcClient::send_and_confirm_transaction`.
///
/// Measured empirically: that SDK method clustered every move's round-trip at
/// ~800ms regardless of how fast the ER itself processed it. Traced to
/// `solana-rpc-client` 3.1.12's nonblocking `send_and_confirm_transaction`,
/// which does `sleep(Duration::from_millis(500))` between each
/// `get_signature_status` poll — so unless the transaction happens to already
/// be confirmed on the very first check (it almost never is), you eat a full
/// extra 500ms tick no matter how fast confirmation actually landed. Polling
/// every 20ms instead removes that artificial floor; the remaining latency is
/// real network/ER time, not client-side waiting.
pub fn fast_send_and_confirm(
    rpc: &RpcClient,
    tx: &solana_sdk::transaction::Transaction,
) -> solana_client::client_error::Result<solana_sdk::signature::Signature> {
    use solana_client::rpc_request::RpcError;

    let signature = rpc.send_transaction(tx)?;
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    let poll_interval = std::time::Duration::from_millis(20);

    loop {
        let statuses = rpc.get_signature_statuses(&[signature])?;
        if let Some(Some(status)) = statuses.value.first() {
            if let Some(err) = &status.err {
                return Err(
                    RpcError::ForUser(format!("transaction {signature} failed: {err}")).into(),
                );
            }
            if status.satisfies_commitment(CommitmentConfig::confirmed()) {
                return Ok(signature);
            }
        }
        if start.elapsed() > timeout {
            return Err(RpcError::ForUser(format!(
                "timed out waiting for {signature} to confirm after {:?}",
                start.elapsed()
            ))
            .into());
        }
        std::thread::sleep(poll_interval);
    }
}

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
    solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(
        micro_lamports,
    )
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

/// `Tournament.status` discriminant for `TournamentStatus::Completed` (see
/// `state/tournament.rs`'s enum order: Registration, Active, Completed,
/// Closed, Cancelled — Borsh encodes fieldless enums as a `u8` in declaration
/// order).
pub const TOURNAMENT_STATUS_COMPLETED: u8 = 2;

/// Minimal sequential Borsh reader for account data. Unlike `fetch_profile_elo`
/// above, `Tournament` has a variable-length `name: String` before the fields
/// we need, so a fixed byte offset doesn't work — this walks the account in
/// field-declaration order instead, skipping what it doesn't need.
struct BorshCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BorshCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> anyhow::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| anyhow::anyhow!("borsh cursor: offset overflow"))?;
        let slice = self.data.get(self.pos..end).ok_or_else(|| {
            anyhow::anyhow!(
                "borsh cursor: out of bounds reading {n} bytes at {}",
                self.pos
            )
        })?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> anyhow::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> anyhow::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> anyhow::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> anyhow::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn i64(&mut self) -> anyhow::Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn pubkey(&mut self) -> anyhow::Result<Pubkey> {
        Ok(Pubkey::new_from_array(self.take(32)?.try_into().unwrap()))
    }

    fn string(&mut self) -> anyhow::Result<()> {
        let len = self.u32()? as usize;
        self.take(len)?;
        Ok(())
    }

    fn option_i64(&mut self) -> anyhow::Result<Option<i64>> {
        if self.u8()? == 1 {
            Ok(Some(self.i64()?))
        } else {
            Ok(None)
        }
    }

    fn option_pubkey(&mut self) -> anyhow::Result<Option<Pubkey>> {
        if self.u8()? == 1 {
            Ok(Some(self.pubkey()?))
        } else {
            Ok(None)
        }
    }
}

/// Fetch a tournament's `status` plus its 1st/2nd/3rd place winners. Follows
/// `state/tournament.rs`'s exact field declaration order — `winner`,
/// `second_place`, `third_place` sit after several fixed- and variable-length
/// fields (including the `name: String`), so this has to parse sequentially
/// rather than jump to a fixed offset.
pub fn fetch_tournament_places(
    rpc: &RpcClient,
    program_id: Pubkey,
    tournament_id: u64,
) -> anyhow::Result<(u8, [Option<Pubkey>; 3])> {
    let tournament_pda =
        Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &program_id).0;
    let data = rpc.get_account_data(&tournament_pda)?;
    let mut c = BorshCursor::new(&data);

    c.take(8)?; // Anchor account discriminator
    c.u64()?; // tournament_id
    c.pubkey()?; // authority
    c.string()?; // name
    c.u64()?; // entry_fee
    c.u64()?; // platform_fee
    c.u64()?; // prize_pool
    c.u16()?; // max_players
    c.u16()?; // player_count
    c.u16()?; // num_registered_players
    let status = c.u8()?; // status (TournamentStatus)
    c.option_i64()?; // start_time
    c.option_i64()?; // end_time
    c.u64()?; // fees_advanced
    c.pubkey()?; // fee_payer
    let type_tag = c.u8()?; // tournament_type discriminant (0 = Swiss, 1 = SingleElimination)
    if type_tag == 0 {
        c.u8()?; // Swiss { rounds }
    }
    c.u8()?; // current_round
    c.u8()?; // total_rounds
    c.u16()?; // total_matches
    c.u16()?; // final_match_index
    c.u32()?; // elo_min
    c.u32()?; // elo_max
    c.u16()?; // min_players
    let winner = c.option_pubkey()?;
    let second_place = c.option_pubkey()?;
    let third_place = c.option_pubkey()?;

    Ok((status, [winner, second_place, third_place]))
}
