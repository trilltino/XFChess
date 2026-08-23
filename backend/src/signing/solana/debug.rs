//! Debug utilities for Solana transactions
//!
//! Provides detailed transaction inspection and error analysis.

use serde::{Deserialize, Serialize};

/// Debug information for a transaction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionDebugInfo {
    pub signature: String,
    pub slot: u64,
    pub timestamp: Option<i64>,
    pub success: bool,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub account_changes: Vec<AccountChange>,
    pub compute_units_consumed: Option<u64>,
    pub fee_paid: u64,
    pub program_ids: Vec<String>,
}

/// Account balance change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountChange {
    pub pubkey: String,
    pub pre_balance: u64,
    pub post_balance: u64,
    pub change: i64,
}

/// Parse program error codes into human-readable messages
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

/// Fetches the real on-chain outcome of `signature` via `getTransaction` and
/// reports what actually happened — `success` reflects the transaction's
/// on-chain error status (`meta.err`), not a hardcoded default. Used by
/// `routes::debug::debug_transaction_endpoint`, the tool an operator reaches
/// for while triaging a live incident, so it must never claim success for a
/// transaction that actually failed (or vice versa).
pub async fn debug_transaction(
    rpc: &solana_client::rpc_client::RpcClient,
    signature: &solana_sdk::signature::Signature,
) -> anyhow::Result<TransactionDebugInfo> {
    use solana_transaction_status::UiTransactionEncoding;

    let confirmed = rpc.get_transaction(signature, UiTransactionEncoding::Base64)?;

    let meta = confirmed.transaction.meta;
    let success = meta.as_ref().is_some_and(|m| m.err.is_none());
    let error = meta
        .as_ref()
        .and_then(|m| m.err.as_ref())
        .map(|e| e.to_string());
    let fee_paid = meta.as_ref().map(|m| m.fee).unwrap_or(0);
    let compute_units_consumed = meta
        .as_ref()
        .and_then(|m| Option::<u64>::from(m.compute_units_consumed.clone()));
    let logs: Vec<String> = meta
        .as_ref()
        .and_then(|m| Option::<Vec<String>>::from(m.log_messages.clone()))
        .unwrap_or_default();

    let account_keys: Vec<String> = confirmed
        .transaction
        .transaction
        .decode()
        .map(|tx| {
            tx.message
                .static_account_keys()
                .iter()
                .map(|k| k.to_string())
                .collect()
        })
        .unwrap_or_default();

    let pre_balances = meta
        .as_ref()
        .map(|m| m.pre_balances.clone())
        .unwrap_or_default();
    let post_balances = meta
        .as_ref()
        .map(|m| m.post_balances.clone())
        .unwrap_or_default();
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

    // Program IDs actually invoked, per the instructions in the decoded
    // message — a subset of `account_keys` in general, but for the small,
    // known instruction set this backend submits, every top-level program_id
    // is one of the static account keys, so indexing into account_keys is
    // sound here.
    let program_ids: Vec<String> = confirmed
        .transaction
        .transaction
        .decode()
        .map(|tx| {
            tx.message
                .instructions()
                .iter()
                .filter_map(|ix| account_keys.get(ix.program_id_index as usize).cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .unwrap_or_default();

    Ok(TransactionDebugInfo {
        signature: signature.to_string(),
        slot: confirmed.slot,
        timestamp: confirmed.block_time,
        success,
        error,
        logs,
        account_changes,
        compute_units_consumed,
        fee_paid,
        program_ids,
    })
}

#[cfg(test)]
/// Try to extract error code from error string.
fn extract_error_code(error_str: &str) -> Option<u32> {
    // Look for patterns like "custom program error: 0x1" or "Custom(1)".
    if let Some(start) = error_str.find("0x") {
        let hex_part = &error_str[start..start + 3.min(error_str.len() - start)];
        if let Ok(code) = u32::from_str_radix(&hex_part[2..], 16) {
            return Some(code);
        }
    }

    if let Some(start) = error_str.find("Custom(") {
        let num_part = &error_str[start + 7..error_str.len() - 1];
        if let Ok(code) = num_part.parse::<u32>() {
            return Some(code);
        }
    }

    None
}

/// Format debug info for human-readable output
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
        if info.success { " Success" } else { " Failed" }
    ));

    if let Some(ref error) = info.error {
        output.push_str(&format!("\nError:\n  {}\n", error));
    }

    if let Some(cu) = info.compute_units_consumed {
        output.push_str(&format!("\nCompute Units: {}\n", cu));
    }

    output.push_str(&format!(
        "Fee Paid: {} lamports ({} SOL)\n",
        info.fee_paid,
        info.fee_paid as f64 / 1_000_000_000.0
    ));

    if !info.account_changes.is_empty() {
        output.push_str("\nAccount Changes:\n");
        for change in &info.account_changes {
            let change_sol = change.change as f64 / 1_000_000_000.0;
            let sign = if change.change >= 0 { "+" } else { "" };
            output.push_str(&format!(
                "  {}: {} ? {} ({}{} SOL)\n",
                &change.pubkey[..8],
                change.pre_balance,
                change.post_balance,
                sign,
                change_sol
            ));
        }
    }

    if !info.program_ids.is_empty() {
        output.push_str("\nPrograms Invoked:\n");
        for (idx, program_id) in info.program_ids.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", idx + 1, program_id));
        }
    }

    if !info.logs.is_empty() {
        output.push_str("\nProgram Logs:\n");
        for log in &info.logs {
            // Indent logs for readability
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
}
