//! Debug utilities for Solana transactions
//!
//! Provides detailed transaction inspection and error analysis.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use solana_transaction_status::{
    UiTransactionEncoding, UiTransactionStatusMeta, EncodedConfirmedTransactionWithStatusMeta,
};
use std::str::FromStr;

/// Debug information for a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Get detailed debug information for a transaction
pub async fn debug_transaction(
    rpc: &RpcClient,
    signature: &Signature,
) -> Result<TransactionDebugInfo> {
    let tx_info = rpc
        .get_transaction_with_config(
            signature,
            solana_client::rpc_config::RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            },
        )
        .map_err(|e| anyhow!("Failed to fetch transaction: {}", e))?;
    
    let meta = tx_info.transaction.meta.as_ref();
    
    // Extract account changes
    let account_changes = extract_account_changes(&tx_info);
    
    // Extract program IDs from transaction
    let program_ids = extract_program_ids(&tx_info);
    
    // Get error if transaction failed
    let error = meta.and_then(|m| {
        m.err.as_ref().map(|e| {
            // Try to parse program error code
            let error_str = format!("{:?}", e);
            if let Some(code) = extract_error_code(&error_str) {
                format!("{} (code: 0x{:X})", parse_program_error(code), code)
            } else {
                error_str
            }
        })
    });
    
    // Get logs
    let logs = meta
        .and_then(|m| m.log_messages.clone())
        .unwrap_or_default();
    
    Ok(TransactionDebugInfo {
        signature: signature.to_string(),
        slot: tx_info.slot,
        timestamp: tx_info.block_time,
        success: error.is_none(),
        error,
        logs,
        account_changes,
        compute_units_consumed: meta.and_then(|m| m.compute_units_consumed),
        fee_paid: meta.map(|m| m.fee).unwrap_or(0),
        program_ids,
    })
}

/// Extract account balance changes from transaction
fn extract_account_changes(
    tx_info: &EncodedConfirmedTransactionWithStatusMeta,
) -> Vec<AccountChange> {
    let meta = match tx_info.transaction.meta.as_ref() {
        Some(m) => m,
        None => return vec![],
    };
    
    let account_keys = tx_info
        .transaction
        .transaction
        .decode()
        .map(|tx| tx.message.static_account_keys().to_vec())
        .unwrap_or_default();
    
    let pre_balances = &meta.pre_balances;
    let post_balances = &meta.post_balances;
    
    account_keys
        .iter()
        .enumerate()
        .filter_map(|(idx, pubkey)| {
            let pre = pre_balances.get(idx).copied().unwrap_or(0);
            let post = post_balances.get(idx).copied().unwrap_or(0);
            let change = post as i64 - pre as i64;
            
            // Only include accounts that changed
            if change != 0 {
                Some(AccountChange {
                    pubkey: pubkey.to_string(),
                    pre_balance: pre,
                    post_balance: post,
                    change,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Extract program IDs from transaction
fn extract_program_ids(
    tx_info: &EncodedConfirmedTransactionWithStatusMeta,
) -> Vec<String> {
    tx_info
        .transaction
        .transaction
        .decode()
        .map(|tx| {
            tx.message
                .instructions()
                .iter()
                .filter_map(|ix| {
                    tx.message
                        .static_account_keys()
                        .get(ix.program_id_index as usize)
                        .map(|pk| pk.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Try to extract error code from error string
fn extract_error_code(error_str: &str) -> Option<u32> {
    // Look for patterns like "custom program error: 0x1" or "Custom(1)"
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
    
    output.push_str(&format!("Status: {}\n", if info.success { "✓ Success" } else { "✗ Failed" }));
    
    if let Some(ref error) = info.error {
        output.push_str(&format!("\nError:\n  {}\n", error));
    }
    
    if let Some(cu) = info.compute_units_consumed {
        output.push_str(&format!("\nCompute Units: {}\n", cu));
    }
    
    output.push_str(&format!("Fee Paid: {} lamports ({} SOL)\n", 
        info.fee_paid, 
        info.fee_paid as f64 / 1_000_000_000.0
    ));
    
    if !info.account_changes.is_empty() {
        output.push_str("\nAccount Changes:\n");
        for change in &info.account_changes {
            let change_sol = change.change as f64 / 1_000_000_000.0;
            let sign = if change.change >= 0 { "+" } else { "" };
            output.push_str(&format!(
                "  {}: {} → {} ({}{} SOL)\n",
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
        assert_eq!(parse_program_error(0x1), "GameAlreadyFull - Game already has two players");
        assert_eq!(parse_program_error(0x2), "InvalidMove - Move is not valid for current position");
        assert_eq!(parse_program_error(0xFF), "Unknown program error");
    }
    
    #[test]
    fn test_extract_error_code() {
        assert_eq!(extract_error_code("custom program error: 0x1"), Some(1));
        assert_eq!(extract_error_code("Custom(5)"), Some(5));
        assert_eq!(extract_error_code("no error code"), None);
    }
}
