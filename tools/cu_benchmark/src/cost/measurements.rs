use serde::{Deserialize, Serialize};
use solana_client::{rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::signature::Signature;
use solana_transaction_status::{option_serializer::OptionSerializer, UiTransactionEncoding};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionLayer {
    Solana,
    Er,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub group: String,
    pub instruction: String,
    pub layer: ExecutionLayer,
    pub cu_consumed: Option<u64>,
    pub cu_limit: Option<u32>,
    pub cu_price_micro_lamports: Option<u64>,
    pub fee_lamports: Option<u64>,
    pub success: bool,
    pub signature: Option<String>,
    pub fee_payer: Option<String>,
    pub pre_balance_lamports: Option<u64>,
    pub post_balance_lamports: Option<u64>,
}

#[derive(Debug, Default)]
pub struct MeasurementLog {
    entries: Vec<Measurement>,
}

impl From<Vec<Measurement>> for MeasurementLog {
    fn from(entries: Vec<Measurement>) -> Self {
        Self { entries }
    }
}

impl MeasurementLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log(
        &mut self,
        layer: ExecutionLayer,
        group: &str,
        instruction: &str,
        success: bool,
        signature: Option<String>,
    ) {
        self.entries.push(Measurement {
            group: group.into(),
            instruction: instruction.into(),
            layer,
            cu_consumed: None,
            cu_limit: None,
            cu_price_micro_lamports: None,
            fee_lamports: None,
            success,
            signature,
            fee_payer: None,
            pre_balance_lamports: None,
            post_balance_lamports: None,
        });
    }

    /// Missing landed metadata stays unknown rather than becoming an estimate.
    pub fn measure(&mut self, base_rpc: &RpcClient, er_rpc: &RpcClient) -> Vec<String> {
        let mut warnings = Vec::new();
        let mut seen = HashSet::new();
        self.entries.retain(|entry| {
            entry
                .signature
                .as_ref()
                .is_none_or(|s| seen.insert(s.clone()))
        });
        for entry in &mut self.entries {
            let Some(signature) = &entry.signature else {
                continue;
            };
            let result = (|| -> anyhow::Result<()> {
                let rpc = match entry.layer {
                    ExecutionLayer::Solana => base_rpc,
                    ExecutionLayer::Er => er_rpc,
                };
                let tx = rpc.get_transaction_with_config(
                    &signature.parse::<Signature>()?,
                    RpcTransactionConfig {
                        encoding: Some(UiTransactionEncoding::Base64),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                    },
                )?;
                let decoded = tx
                    .transaction
                    .transaction
                    .decode()
                    .ok_or_else(|| anyhow::anyhow!("transaction decoding failed"))?;
                let meta = tx
                    .transaction
                    .meta
                    .ok_or_else(|| anyhow::anyhow!("metadata absent"))?;
                entry.fee_lamports = Some(meta.fee);
                entry.success = meta.err.is_none();
                if let OptionSerializer::Some(cu) = meta.compute_units_consumed {
                    entry.cu_consumed = Some(cu);
                }
                let keys = decoded.message.static_account_keys();
                entry.fee_payer = keys.first().map(ToString::to_string);
                entry.pre_balance_lamports = meta.pre_balances.first().copied();
                entry.post_balance_lamports = meta.post_balances.first().copied();
                for ix in decoded.message.instructions() {
                    if keys.get(ix.program_id_index as usize)
                        != Some(&solana_compute_budget_interface::id())
                    {
                        continue;
                    }
                    match ix.data.as_slice() {
                        [2, a, b, c, d] => {
                            entry.cu_limit = Some(u32::from_le_bytes([*a, *b, *c, *d]))
                        }
                        [3, a, b, c, d, e, f, g, h] => {
                            entry.cu_price_micro_lamports =
                                Some(u64::from_le_bytes([*a, *b, *c, *d, *e, *f, *g, *h]))
                        }
                        _ => {}
                    }
                }
                Ok(())
            })();
            if result.is_err() {
                warnings.push(format!(
                    "{}: metadata unavailable for {signature}",
                    entry.instruction
                ));
            }
        }
        warnings
    }

    pub fn total_cu(&self) -> u64 {
        self.entries
            .iter()
            .filter_map(|entry| entry.cu_consumed)
            .sum()
    }

    pub fn print_summary(&self) {
        let mut groups: BTreeMap<&str, (u64, usize, usize)> = BTreeMap::new();
        for entry in &self.entries {
            let group = groups.entry(&entry.group).or_default();
            group.0 += entry.cu_consumed.unwrap_or(0);
            group.1 += usize::from(entry.cu_consumed.is_some());
            group.2 += usize::from(entry.signature.is_some());
        }
        println!("\n{:<28} {:>12} {:>14}", "Group", "Measured CU", "Coverage");
        for (name, (cu, measured, submitted)) in groups {
            println!("{name:<28} {cu:>12} {measured:>6}/{submitted:<6}");
        }
    }

    pub fn entries(&self) -> &[Measurement] {
        &self.entries
    }
}
