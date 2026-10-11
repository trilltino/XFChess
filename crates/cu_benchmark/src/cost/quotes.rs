use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use solana_client::{rpc_client::RpcClient, rpc_request::RpcRequest};
use solana_sdk::pubkey::Pubkey;

use crate::cost::estimate::Workload;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrioritySample {
    pub slot: u64,
    pub prioritization_fee: u64,
}

#[derive(Debug, Serialize)]
pub struct PriorityQuote {
    pub percentile: u16,
    pub writable_accounts: Vec<String>,
    pub micro_lamports_per_cu: u64,
    pub samples: Vec<PrioritySample>,
}

pub fn quote_rent(rpc: &RpcClient, workload: &mut Workload) -> Result<()> {
    let mut rents = std::collections::HashMap::new();
    for variant in &mut workload.variants {
        for account in &mut variant.accounts {
            let rent = match rents.get(&account.data_bytes) {
                Some(rent) => *rent,
                None => {
                    let rent = rpc.get_minimum_balance_for_rent_exemption(account.data_bytes)?;
                    rents.insert(account.data_bytes, rent);
                    rent
                }
            };
            account.rent_lamports = Some(rent);
        }
    }
    Ok(())
}

pub fn priority_quote(
    rpc: &RpcClient,
    percentile: u16,
    accounts: &[Pubkey],
) -> Result<PriorityQuote> {
    ensure!(
        (1..=10_000).contains(&percentile),
        "priority percentile must be 1..=10000"
    );
    ensure!(
        !accounts.is_empty(),
        "supply --writable-account for a transaction-specific priority quote"
    );
    let writable_accounts: Vec<String> = accounts.iter().map(ToString::to_string).collect();
    let samples: Vec<PrioritySample> = rpc.send(
        RpcRequest::GetRecentPrioritizationFees,
        serde_json::json!([writable_accounts, {"percentile": percentile}]),
    )?;
    let mut prices: Vec<_> = samples
        .iter()
        .map(|sample| sample.prioritization_fee)
        .collect();
    prices.sort_unstable();
    let price = prices
        .get(prices.len() / 2)
        .copied()
        .context("RPC returned no priority fee samples")?;
    Ok(PriorityQuote {
        percentile,
        writable_accounts,
        micro_lamports_per_cu: price,
        samples,
    })
}
