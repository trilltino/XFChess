use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::{BTreeSet, HashMap};

use crate::signing::solana::{fetch_transaction_v1_aware, MAX_SUPPORTED_TX_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionDebugInfo {
    pub signature: String,
    pub slot: u64,
    pub timestamp: Option<i64>,
    pub status: TransactionStatusSummary,
    pub success: bool,
    pub error: Option<String>,
    pub metadata: TransactionMetadata,
    pub outer_instructions: Vec<InstructionDebugInfo>,
    pub failing_instruction: Option<InstructionDebugInfo>,
    pub cpi_tree: Vec<CpiFrame>,
    pub accounts: Vec<AccountEvidence>,
    pub rent_evidence: Vec<RentEvidence>,
    pub logs: Vec<String>,
    pub account_changes: Vec<AccountChange>,
    pub compute_units_consumed: Option<u64>,
    pub fee_paid: u64,
    pub program_ids: Vec<String>,
    pub freshness: FreshnessInfo,
    pub root_cause: RootCause,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionStatusSummary {
    pub landed: bool,
    pub finalized: bool,
    pub err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionMetadata {
    pub payer: Option<String>,
    pub recent_blockhash: Option<String>,
    pub required_signatures: usize,
    pub readonly_signed_accounts: u8,
    pub readonly_unsigned_accounts: u8,
    pub transaction_version: String,
    pub max_supported_transaction_version: u8,
    pub rpc_v1_fetch_supported: bool,
    pub transaction_size_bytes: Option<usize>,
    pub uses_address_lookup_tables: bool,
    pub static_account_count: usize,
    pub loaded_writable_account_count: usize,
    pub loaded_readonly_account_count: usize,
    pub v1_compute_unit_limit: Option<u64>,
    pub v1_loaded_accounts_data_size_limit: Option<u64>,
    pub fetch_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionDebugInfo {
    pub index: usize,
    pub program_id: String,
    pub program_label: String,
    pub account_indexes: Vec<u8>,
    pub accounts: Vec<InstructionAccountMeta>,
    pub data_base58: String,
    pub discriminator: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionAccountMeta {
    pub index: usize,
    pub pubkey: String,
    pub signer: bool,
    pub writable: bool,
    pub owner: Option<String>,
    pub owner_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEvidence {
    pub index: usize,
    pub pubkey: String,
    pub owner: Option<String>,
    pub owner_label: Option<String>,
    pub executable: Option<bool>,
    pub lamports_pre: Option<u64>,
    pub lamports_post: Option<u64>,
    pub lamports_change: Option<i64>,
    pub data_len: Option<usize>,
    pub signer: bool,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RentEvidence {
    pub pubkey: String,
    pub lamports: u64,
    pub data_len: usize,
    pub rent_exempt_minimum: Option<u64>,
    pub reclaimable_surplus: Option<u64>,
    pub below_rent_exempt: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpiFrame {
    pub depth: usize,
    pub program_id: String,
    pub program_label: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountChange {
    pub pubkey: String,
    pub pre_balance: u64,
    pub post_balance: u64,
    pub change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FreshnessInfo {
    pub execution_slot: u64,
    pub current_slot: Option<u64>,
    pub slot_age: Option<u64>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    pub category: String,
    pub summary: String,
    pub evidence: Vec<String>,
}

impl Default for RootCause {
    fn default() -> Self {
        Self {
            category: "unknown".to_string(),
            summary: "No failure evidence was available.".to_string(),
            evidence: Vec::new(),
        }
    }
}

pub fn parse_program_error(code: u32) -> &'static str {
    match code {
        0x0 => "Success",
        0x1 => "GameAlreadyFull - Game already has two players",
        0x2 => "InvalidMove - Move is not valid for current position",
        0x3 => "NotYourTurn - Attempted to move out of turn",
        0x4 => "GameNotActive - Game is not in active state",
        0x5 => "Unauthorized - Signer is not authorized",
        0x6 => "InvalidState - Game is in invalid state for operation",
        0x7 => "Timeout - Operation timed out",
        0x8 => "InvalidWager - Wager amount is invalid",
        0x9 => "InsufficientEscrow - Escrow has insufficient funds",
        0xA => "GameNotFinished - Game must be finished to claim",
        0xB => "AlreadyClaimed - Rewards already claimed",
        _ => "Unknown program error",
    }
}

pub async fn debug_transaction(
    rpc: &solana_client::rpc_client::RpcClient,
    signature: &solana_sdk::signature::Signature,
) -> anyhow::Result<TransactionDebugInfo> {
    let fetched = fetch_transaction_v1_aware(rpc, signature)?;
    let confirmed = &fetched.confirmed;
    let meta = confirmed.transaction.meta.as_ref();
    let success = meta.is_some_and(|m| m.err.is_none());
    let error = meta.and_then(|m| m.err.as_ref()).map(|e| e.to_string());
    let failing_index = error.as_deref().and_then(failing_instruction_index);
    let fee_paid = meta.map(|m| m.fee).unwrap_or(0);
    let compute_units_consumed =
        meta.and_then(|m| Option::<u64>::from(m.compute_units_consumed.clone()));
    let logs: Vec<String> = meta
        .and_then(|m| Option::<Vec<String>>::from(m.log_messages.clone()))
        .unwrap_or_default();

    let decoded = Some(fetched.decoded.clone());
    let account_pubkeys: Vec<Pubkey> = fetched.static_account_keys.clone();
    let account_keys: Vec<String> = account_pubkeys.iter().map(|k| k.to_string()).collect();
    let account_infos = if account_pubkeys.is_empty() {
        Vec::new()
    } else {
        rpc.get_multiple_accounts(&account_pubkeys)
            .unwrap_or_default()
    };

    let pre_balances = meta.map(|m| m.pre_balances.clone()).unwrap_or_default();
    let post_balances = meta.map(|m| m.post_balances.clone()).unwrap_or_default();
    let account_changes: Vec<AccountChange> = account_keys
        .iter()
        .zip(pre_balances.iter())
        .zip(post_balances.iter())
        .map(|((pubkey, &pre), &post)| AccountChange {
            pubkey: pubkey.clone(),
            pre_balance: pre,
            post_balance: post,
            change: post as i64 - pre as i64,
        })
        .collect();

    let metadata = decoded
        .as_ref()
        .map(|tx| {
            let header = tx.message.header();
            let (v1_compute_unit_limit, v1_loaded_accounts_data_size_limit) = v1_resource_limits(
                tx.message
                    .instructions()
                    .iter()
                    .map(|ix| (ix.program_id_index, ix.data.as_slice())),
                &account_keys,
            );
            TransactionMetadata {
                payer: account_keys.first().cloned(),
                recent_blockhash: Some(tx.message.recent_blockhash().to_string()),
                required_signatures: header.num_required_signatures as usize,
                readonly_signed_accounts: header.num_readonly_signed_accounts,
                readonly_unsigned_accounts: header.num_readonly_unsigned_accounts,
                transaction_version: fetched.version.clone(),
                max_supported_transaction_version: MAX_SUPPORTED_TX_VERSION,
                rpc_v1_fetch_supported: fetched.rpc_v1_fetch_supported,
                transaction_size_bytes: fetched.transaction_size_bytes,
                uses_address_lookup_tables: tx
                    .message
                    .address_table_lookups()
                    .is_some_and(|lookups| !lookups.is_empty()),
                static_account_count: account_keys.len(),
                loaded_writable_account_count: fetched.loaded_writable_accounts.len(),
                loaded_readonly_account_count: fetched.loaded_readonly_accounts.len(),
                v1_compute_unit_limit,
                v1_loaded_accounts_data_size_limit,
                fetch_warnings: fetched.fetch_warnings.clone(),
            }
        })
        .unwrap_or_default();

    let accounts = build_account_evidence(
        &account_keys,
        &account_infos,
        &pre_balances,
        &post_balances,
        metadata.required_signatures,
        metadata.readonly_signed_accounts,
        metadata.readonly_unsigned_accounts,
    );
    let rent_evidence = build_rent_evidence(rpc, &account_keys, &account_infos);

    let outer_instructions: Vec<InstructionDebugInfo> = decoded
        .as_ref()
        .map(|tx| {
            tx.message
                .instructions()
                .iter()
                .enumerate()
                .map(|(idx, ix)| {
                    let program_id = account_keys
                        .get(ix.program_id_index as usize)
                        .cloned()
                        .unwrap_or_else(|| format!("account_index_{}", ix.program_id_index));
                    let error = if Some(idx) == failing_index {
                        meta.and_then(|m| m.err.as_ref()).map(|e| e.to_string())
                    } else {
                        None
                    };
                    InstructionDebugInfo {
                        index: idx,
                        program_label: program_label(&program_id).to_string(),
                        accounts: ix
                            .accounts
                            .iter()
                            .map(|account_index| {
                                let index = *account_index as usize;
                                let owner = account_infos
                                    .get(index)
                                    .and_then(|a| a.as_ref())
                                    .map(|a| a.owner.to_string());
                                InstructionAccountMeta {
                                    index,
                                    pubkey: account_keys
                                        .get(index)
                                        .cloned()
                                        .unwrap_or_else(|| format!("account_index_{index}")),
                                    signer: is_signer(index, metadata.required_signatures),
                                    writable: is_writable(
                                        index,
                                        account_keys.len(),
                                        metadata.required_signatures,
                                        metadata.readonly_signed_accounts,
                                        metadata.readonly_unsigned_accounts,
                                    ),
                                    owner_label: owner
                                        .as_deref()
                                        .map(program_label)
                                        .map(str::to_string),
                                    owner,
                                }
                            })
                            .collect(),
                        account_indexes: ix.accounts.clone(),
                        discriminator: instruction_discriminator(&ix.data),
                        data_base58: bs58::encode(&ix.data).into_string(),
                        error,
                        program_id,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let failing_instruction = failing_index.and_then(|idx| outer_instructions.get(idx).cloned());
    let program_ids: Vec<String> = outer_instructions
        .iter()
        .map(|ix| ix.program_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let current_slot = rpc.get_slot().ok();
    let freshness = FreshnessInfo {
        execution_slot: confirmed.slot,
        current_slot,
        slot_age: current_slot.map(|s| s.saturating_sub(confirmed.slot)),
        note: freshness_note(current_slot.map(|s| s.saturating_sub(confirmed.slot))),
    };
    let cpi_tree = parse_cpi_tree(&logs);
    let root_cause = classify_root_cause(
        success,
        error.as_deref(),
        failing_instruction.as_ref(),
        &logs,
        compute_units_consumed,
        &metadata,
        &freshness,
    );
    let recommended_actions = recommended_actions(&root_cause, &failing_instruction, success);

    Ok(TransactionDebugInfo {
        signature: signature.to_string(),
        slot: confirmed.slot,
        timestamp: confirmed.block_time,
        status: TransactionStatusSummary {
            landed: true,
            finalized: true,
            err: error.clone(),
        },
        success,
        error,
        metadata,
        outer_instructions,
        failing_instruction,
        cpi_tree,
        accounts,
        rent_evidence,
        logs,
        account_changes,
        compute_units_consumed,
        fee_paid,
        program_ids,
        freshness,
        root_cause,
        recommended_actions,
    })
}

fn failing_instruction_index(err: &str) -> Option<usize> {
    let marker = "InstructionError(";
    let start = err.find(marker)? + marker.len();
    let tail = &err[start..];
    let end = tail.find(',')?;
    tail[..end].trim().parse::<usize>().ok()
}

fn instruction_discriminator(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        None
    } else {
        Some(hex::encode(&data[..data.len().min(8)]))
    }
}

fn is_signer(index: usize, required_signatures: usize) -> bool {
    index < required_signatures
}

fn is_writable(
    index: usize,
    account_count: usize,
    required_signatures: usize,
    readonly_signed: u8,
    readonly_unsigned: u8,
) -> bool {
    if index >= account_count {
        return false;
    }
    if index < required_signatures {
        index < required_signatures.saturating_sub(readonly_signed as usize)
    } else {
        index < account_count.saturating_sub(readonly_unsigned as usize)
    }
}

fn build_account_evidence(
    account_keys: &[String],
    account_infos: &[Option<solana_sdk::account::Account>],
    pre_balances: &[u64],
    post_balances: &[u64],
    required_signatures: usize,
    readonly_signed_accounts: u8,
    readonly_unsigned_accounts: u8,
) -> Vec<AccountEvidence> {
    account_keys
        .iter()
        .enumerate()
        .map(|(index, pubkey)| {
            let info = account_infos.get(index).and_then(|a| a.as_ref());
            let owner = info.map(|a| a.owner.to_string());
            let pre = pre_balances.get(index).copied();
            let post = post_balances.get(index).copied();
            AccountEvidence {
                index,
                pubkey: pubkey.clone(),
                owner_label: owner.as_deref().map(program_label).map(str::to_string),
                owner,
                executable: info.map(|a| a.executable),
                data_len: info.map(|a| a.data.len()),
                lamports_pre: pre,
                lamports_post: post,
                lamports_change: pre.zip(post).map(|(a, b)| b as i64 - a as i64),
                signer: is_signer(index, required_signatures),
                writable: is_writable(
                    index,
                    account_keys.len(),
                    required_signatures,
                    readonly_signed_accounts,
                    readonly_unsigned_accounts,
                ),
            }
        })
        .collect()
}

fn build_rent_evidence(
    rpc: &solana_client::rpc_client::RpcClient,
    account_keys: &[String],
    account_infos: &[Option<solana_sdk::account::Account>],
) -> Vec<RentEvidence> {
    let mut minimum_by_len = HashMap::<usize, Option<u64>>::new();
    account_keys
        .iter()
        .zip(account_infos.iter())
        .filter_map(|(pubkey, info)| {
            let info = info.as_ref()?;
            let data_len = info.data.len();
            let minimum = *minimum_by_len
                .entry(data_len)
                .or_insert_with(|| rpc.get_minimum_balance_for_rent_exemption(data_len).ok());
            Some(RentEvidence {
                pubkey: pubkey.clone(),
                lamports: info.lamports,
                data_len,
                rent_exempt_minimum: minimum,
                reclaimable_surplus: minimum.map(|min| info.lamports.saturating_sub(min)),
                below_rent_exempt: minimum.map(|min| info.lamports < min),
            })
        })
        .collect()
}

fn v1_resource_limits<'a>(
    instructions: impl IntoIterator<Item = (u8, &'a [u8])>,
    account_keys: &[String],
) -> (Option<u64>, Option<u64>) {
    let mut compute_unit_limit = None;
    let mut loaded_accounts_data_size_limit = None;
    for (program_id_index, data) in instructions {
        let Some(program_id) = account_keys.get(program_id_index as usize) else {
            continue;
        };
        if program_id != "ComputeBudget111111111111111111111111111111" {
            continue;
        }
        match data {
            [2, rest @ ..] if rest.len() >= 4 => {
                compute_unit_limit =
                    Some(u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as u64);
            }
            [4, rest @ ..] if rest.len() >= 4 => {
                loaded_accounts_data_size_limit =
                    Some(u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as u64);
            }
            _ => {}
        }
    }
    (compute_unit_limit, loaded_accounts_data_size_limit)
}

fn program_label(program_id: &str) -> &'static str {
    match program_id {
        "11111111111111111111111111111111" => "System Program",
        "ComputeBudget111111111111111111111111111111" => "Compute Budget",
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr" => "Memo",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" => "SPL Token",
        "TokenzQdBNbLqP5VEhdkAS6EPF8jkS2Yd9bQ2gX5m" => "Token-2022",
        "ATokenGPvoter1pJJV7TH4TH7ma9aWQ2eyczB6hNzE" => "Associated Token",
        "675kPX9MHTjS2zt1qfr1NYFnAFkL5MP8gy1pjxP5M" => "Raydium AMM",
        "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK" => "Raydium CLMM",
        "CPMMoo8L3F4NbTegBCKVN5hM6KcpCwxo1zqyA3CcuF1" => "Raydium CPMM",
        _ => "Unknown Program",
    }
}

fn parse_cpi_tree(logs: &[String]) -> Vec<CpiFrame> {
    logs.iter()
        .filter_map(|line| {
            let rest = line.strip_prefix("Program ")?;
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "invoke" {
                let depth = parts
                    .get(2)
                    .and_then(|d| d.trim_matches(['[', ']']).parse::<usize>().ok())
                    .unwrap_or(1);
                return Some(CpiFrame {
                    depth,
                    program_id: parts[0].to_string(),
                    program_label: program_label(parts[0]).to_string(),
                    status: "invoke".to_string(),
                    message: line.clone(),
                });
            }
            if parts.len() >= 2 && (parts[1] == "success" || parts[1] == "failed:") {
                return Some(CpiFrame {
                    depth: 0,
                    program_id: parts[0].to_string(),
                    program_label: program_label(parts[0]).to_string(),
                    status: if parts[1] == "success" {
                        "success"
                    } else {
                        "failed"
                    }
                    .to_string(),
                    message: line.clone(),
                });
            }
            None
        })
        .collect()
}

fn freshness_note(slot_age: Option<u64>) -> String {
    match slot_age {
        Some(age) if age > 10_000 => {
            format!("Execution slot is {age} slots behind the current RPC slot; compare this with quote/discovery slot before retrying.")
        }
        Some(age) => format!("Execution slot is {age} slots behind the current RPC slot."),
        None => "Current slot unavailable; freshness could not be quantified.".to_string(),
    }
}

fn classify_root_cause(
    success: bool,
    error: Option<&str>,
    failing_instruction: Option<&InstructionDebugInfo>,
    logs: &[String],
    compute_units: Option<u64>,
    metadata: &TransactionMetadata,
    freshness: &FreshnessInfo,
) -> RootCause {
    if success {
        return RootCause {
            category: "landed_successfully".to_string(),
            summary: "The transaction landed successfully. No on-chain failure was found."
                .to_string(),
            evidence: vec!["Transaction metadata has no error.".to_string()],
        };
    }

    let joined_logs = logs.join("\n").to_ascii_lowercase();
    let err = error.unwrap_or("").to_ascii_lowercase();
    let mut evidence = Vec::new();
    if let Some(ix) = failing_instruction {
        evidence.push(format!(
            "Instruction #{} failed in {} ({})",
            ix.index, ix.program_label, ix.program_id
        ));
    }
    if let Some(raw) = error {
        evidence.push(format!("Raw Solana error: {raw}"));
    }

    if err.contains("blockhash") || err.contains("not found") {
        return RootCause {
            category: "rpc_submission".to_string(),
            summary: "The failure is consistent with transaction submission, blockhash, or confirmation state.".to_string(),
            evidence,
        };
    }
    if joined_logs.contains("computational budget exceeded")
        || joined_logs.contains("compute budget exceeded")
        || err.contains("computationalbudgetexceeded")
    {
        evidence.push(format!("Compute units consumed: {:?}", compute_units));
        if metadata.uses_address_lookup_tables {
            evidence.push(
                "Address lookup tables were used; ALT reduces message size, not compute."
                    .to_string(),
            );
        }
        return RootCause {
            category: "protocol_rejection".to_string(),
            summary: "The transaction exhausted compute or hit a protocol-level execution limit."
                .to_string(),
            evidence,
        };
    }
    if joined_logs.contains("token-2022")
        || failing_instruction.is_some_and(|ix| ix.program_label == "Token-2022")
    {
        return RootCause {
            category: "token_program_incompatibility".to_string(),
            summary: "The failure reached Token-2022; verify extension-specific account requirements for this instruction path.".to_string(),
            evidence,
        };
    }
    if freshness.slot_age.is_some_and(|age| age > 10_000) {
        evidence.push(freshness.note.clone());
        return RootCause {
            category: "stale_state".to_string(),
            summary: "The execution slot is far behind the current observed slot; stale quote or state should be investigated.".to_string(),
            evidence,
        };
    }
    if joined_logs.contains("invalid account")
        || joined_logs.contains("owner")
        || joined_logs.contains("account data")
        || joined_logs.contains("custom program error")
    {
        return RootCause {
            category: "sdk_client_construction".to_string(),
            summary: "The failure looks like an account graph, ownership, layout, or serialized instruction mismatch.".to_string(),
            evidence,
        };
    }

    RootCause {
        category: "unknown".to_string(),
        summary: "The transaction failed, but the available metadata is not enough for a confident classification.".to_string(),
        evidence,
    }
}

fn recommended_actions(
    root_cause: &RootCause,
    failing_instruction: &Option<InstructionDebugInfo>,
    success: bool,
) -> Vec<String> {
    if success {
        return vec![
            "No retry needed. If the client saw a timeout, reconcile by signature before resubmitting."
                .to_string(),
        ];
    }
    let mut actions = vec!["Do not retry blindly. Reconcile by this signature first.".to_string()];
    if let Some(ix) = failing_instruction {
        actions.push(format!(
            "Inspect instruction #{} account order, signer/writable flags, and discriminator {}.",
            ix.index,
            ix.discriminator.as_deref().unwrap_or("n/a")
        ));
    }
    match root_cause.category.as_str() {
        "token_program_incompatibility" => actions.push(
            "Check legacy SPL Token vs Token-2022 mint extensions and required extra accounts."
                .to_string(),
        ),
        "stale_state" => actions.push(
            "Compare quote/discovery slot with execution slot and rebuild from fresh state if needed."
                .to_string(),
        ),
        "protocol_rejection" => actions.push(
            "Separate message-size issues from compute issues; ALT usage will not reduce compute."
                .to_string(),
        ),
        "sdk_client_construction" => actions.push(
            "Validate expected PDA, owner, data length/layout, discriminator, and semantic fields separately."
                .to_string(),
        ),
        _ => actions.push(
            "Use the CPI tree and logs to identify the lowest-level program that rejected execution."
                .to_string(),
        ),
    }
    actions
}

#[cfg(test)]
fn extract_error_code(error_str: &str) -> Option<u32> {
    if let Some(start) = error_str.find("0x") {
        let end = error_str[start + 2..]
            .find(|c: char| !c.is_ascii_hexdigit())
            .map(|offset| start + 2 + offset)
            .unwrap_or(error_str.len());
        if end > start + 2 {
            return u32::from_str_radix(&error_str[start + 2..end], 16).ok();
        }
    }

    if let Some(start) = error_str.find("Custom(") {
        let tail = &error_str[start + 7..];
        let end = tail.find(')').unwrap_or(tail.len());
        return tail[..end].parse::<u32>().ok();
    }

    None
}

pub fn format_debug_info(info: &TransactionDebugInfo) -> String {
    let mut output = String::new();
    output.push_str(&format!("Transaction: {}\n", info.signature));
    output.push_str(&format!("Slot: {}\n", info.slot));

    if let Some(ts) = info.timestamp {
        let datetime = chrono::DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "Unknown".to_string());
        output.push_str(&format!("Time: {}\n", datetime));
    }

    output.push_str(&format!(
        "Status: {}\n",
        if info.success { "Success" } else { "Failed" }
    ));

    if let Some(ref error) = info.error {
        output.push_str(&format!("\nError:\n  {}\n", error));
    }

    if let Some(ix) = &info.failing_instruction {
        output.push_str(&format!(
            "\nFailing Instruction:\n  #{} {} ({})\n",
            ix.index, ix.program_label, ix.program_id
        ));
    }

    output.push_str(&format!(
        "\nRoot Cause:\n  {} - {}\n",
        info.root_cause.category, info.root_cause.summary
    ));

    if let Some(cu) = info.compute_units_consumed {
        output.push_str(&format!("\nCompute Units: {}\n", cu));
    }

    output.push_str(&format!(
        "\nTransaction Version: {}\n",
        info.metadata.transaction_version
    ));
    output.push_str(&format!(
        "RPC maxSupportedTransactionVersion: {}\n",
        info.metadata.max_supported_transaction_version
    ));
    if let Some(size) = info.metadata.transaction_size_bytes {
        output.push_str(&format!("Transaction Size: {} bytes\n", size));
    }
    output.push_str(&format!(
        "Accounts: {} static, {} ALT writable, {} ALT readonly\n",
        info.metadata.static_account_count,
        info.metadata.loaded_writable_account_count,
        info.metadata.loaded_readonly_account_count
    ));
    if info.metadata.transaction_version == "v1" {
        output.push_str(&format!(
            "V1 Resource Limits: compute_unit_limit={}, loaded_accounts_data_size_limit={}\n",
            info.metadata
                .v1_compute_unit_limit
                .map(|v| v.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            info.metadata
                .v1_loaded_accounts_data_size_limit
                .map(|v| v.to_string())
                .unwrap_or_else(|| "missing".to_string())
        ));
    }
    for warning in &info.metadata.fetch_warnings {
        output.push_str(&format!("Fetch Warning: {}\n", warning));
    }

    output.push_str(&format!(
        "Fee Paid: {} lamports ({} SOL)\n",
        info.fee_paid,
        info.fee_paid as f64 / 1_000_000_000.0
    ));

    if !info.rent_evidence.is_empty() {
        output.push_str("\nRent Evidence:\n");
        for rent in info.rent_evidence.iter().take(20) {
            let min = rent
                .rent_exempt_minimum
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unavailable".to_string());
            let surplus = rent
                .reclaimable_surplus
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unavailable".to_string());
            output.push_str(&format!(
                "  {}: lamports={} data_len={} rent_min={} surplus={}\n",
                rent.pubkey, rent.lamports, rent.data_len, min, surplus
            ));
        }
    }

    if !info.program_ids.is_empty() {
        output.push_str("\nPrograms Invoked:\n");
        for (idx, program_id) in info.program_ids.iter().enumerate() {
            output.push_str(&format!(
                "  {}. {} ({})\n",
                idx + 1,
                program_label(program_id),
                program_id
            ));
        }
    }

    if !info.logs.is_empty() {
        output.push_str("\nProgram Logs:\n");
        for log in &info.logs {
            output.push_str(&format!("  {}\n", log));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_program_error() {
        assert_eq!(
            parse_program_error(0x1),
            "GameAlreadyFull - Game already has two players"
        );
        assert_eq!(
            parse_program_error(0x2),
            "InvalidMove - Move is not valid for current position"
        );
        assert_eq!(parse_program_error(0xFF), "Unknown program error");
    }

    #[test]
    fn test_extract_error_code() {
        assert_eq!(extract_error_code("custom program error: 0x1"), Some(1));
        assert_eq!(extract_error_code("Custom(5)"), Some(5));
        assert_eq!(extract_error_code("no error code"), None);
    }

    #[test]
    fn parses_cpi_frames() {
        let frames = parse_cpi_tree(&[
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ]);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].depth, 1);
        assert_eq!(frames[1].status, "success");
    }
}
