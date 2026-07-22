//! Transaction telemetry wrapper for Solana operations
//!
//! Wraps transaction submission with detailed logging, metrics, and error classification.

use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{signature::Signature, transaction::Transaction};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::telemetry::logging::RequestContext;

/// Extended transaction result with telemetry data
#[derive(Debug)]
pub struct TransactionTelemetry {
    pub signature: Signature,
    pub method: String,
    pub chain_type: String,
    pub confirmation_time_ms: Option<u64>,
    pub error: Option<TxErrorDetail>,
}

/// Detailed error information for transaction failures
#[derive(Debug, Clone)]
pub struct TxErrorDetail {
    pub category: TxErrorCategory,
    pub code: Option<u32>,
    pub message: String,
    pub logs: Vec<String>,
}

/// Classification of transaction errors
#[derive(Debug, Clone)]
pub enum TxErrorCategory {
    // Client errors (fix the request)
    InvalidInstruction,
    InsufficientFunds,
    AccountNotFound,
    ProgramError(u32), // Custom program error code

    // Server/transient errors (retry)
    RpcTimeout,
    RpcRateLimit,
    BlockhashExpired,

    // Infrastructure errors (alert ops)
    FeePayerExhausted,
    RpcUnavailable,
    Unknown,
}

impl std::fmt::Display for TxErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxErrorCategory::InvalidInstruction => write!(f, "InvalidInstruction"),
            TxErrorCategory::InsufficientFunds => write!(f, "InsufficientFunds"),
            TxErrorCategory::AccountNotFound => write!(f, "AccountNotFound"),
            TxErrorCategory::ProgramError(code) => write!(f, "ProgramError({})", code),
            TxErrorCategory::RpcTimeout => write!(f, "RpcTimeout"),
            TxErrorCategory::RpcRateLimit => write!(f, "RpcRateLimit"),
            TxErrorCategory::BlockhashExpired => write!(f, "BlockhashExpired"),
            TxErrorCategory::FeePayerExhausted => write!(f, "FeePayerExhausted"),
            TxErrorCategory::RpcUnavailable => write!(f, "RpcUnavailable"),
            TxErrorCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Submit transaction with full telemetry
pub async fn submit_with_telemetry(
    rpc: &RpcClient,
    tx: &Transaction,
    ctx: &RequestContext,
    chain_type: &str,
) -> Result<TransactionTelemetry> {
    let start = Instant::now();
    let method = "send_and_confirm_transaction";

    // Attempt submission
    let result = rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        tx,
        CommitmentConfig::confirmed(),
    );

    let duration = start.elapsed();

    match result {
        Ok(sig) => {
            let confirmation_time_ms = duration.as_millis() as u64;

            info!(
                request_id = %ctx.request_id,
                signature = %sig,
                chain = chain_type,
                confirmation_time_ms = confirmation_time_ms,
                game_id = ctx.game_id.unwrap_or(0),
                "transaction_confirmed"
            );

            Ok(TransactionTelemetry {
                signature: sig,
                method: method.to_string(),
                chain_type: chain_type.to_string(),
                confirmation_time_ms: Some(confirmation_time_ms),
                error: None,
            })
        }
        Err(e) => {
            let error_detail = classify_error(&e);
            let error_str = e.to_string();

            error!(
                request_id = %ctx.request_id,
                error = %error_str,
                error_category = %error_detail.category,
                error_code = error_detail.code.unwrap_or(0),
                chain = chain_type,
                game_id = ctx.game_id.unwrap_or(0),
                "transaction_failed"
            );

            Err(anyhow!("Transaction failed: {}", error_str))
        }
    }
}

/// Submit to Execution Rollup with telemetry
pub async fn submit_er_with_telemetry(
    rpc: &RpcClient,
    tx: &Transaction,
    ctx: &RequestContext,
) -> Result<TransactionTelemetry> {
    use solana_client::rpc_config::RpcSendTransactionConfig;

    let start = Instant::now();
    let method = "send_transaction_er";

    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };

    // Send transaction
    let send_start = Instant::now();
    let sig = match rpc.send_transaction_with_config(tx, config) {
        Ok(sig) => sig,
        Err(e) => {
            let error_detail = classify_error(&e);
            error!(
                request_id = %ctx.request_id,
                error = %e,
                error_category = %error_detail.category,
                "er_send_failed"
            );
            return Err(anyhow!("ER send failed: {}", e));
        }
    };

    info!(
        request_id = %ctx.request_id,
        signature = %sig,
        send_latency_ms = send_start.elapsed().as_millis() as u64,
        "er_transaction_sent"
    );

    // Poll for confirmation
    let confirm_start = Instant::now();
    let commitment = CommitmentConfig::confirmed();
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        if Instant::now() > deadline {
            error!(
                request_id = %ctx.request_id,
                signature = %sig,
                timeout_seconds = 30,
                "er_confirmation_timeout"
            );
            return Err(anyhow!("ER confirmation timeout for {sig}"));
        }

        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => {
                let confirmation_time_ms = confirm_start.elapsed().as_millis() as u64;

                info!(
                    request_id = %ctx.request_id,
                    signature = %sig,
                    confirmation_time_ms = confirmation_time_ms,
                    total_time_ms = start.elapsed().as_millis() as u64,
                    "er_transaction_confirmed"
                );

                return Ok(TransactionTelemetry {
                    signature: sig,
                    method: method.to_string(),
                    chain_type: "er".to_string(),
                    confirmation_time_ms: Some(confirmation_time_ms),
                    error: None,
                });
            }
            Ok(Some(Err(e))) => {
                let error_detail = classify_error_from_status(&e);
                error!(
                    request_id = %ctx.request_id,
                    signature = %sig,
                    error = ?e,
                    error_category = %error_detail.category,
                    "er_transaction_failed_on_chain"
                );
                return Err(anyhow!("ER transaction failed on-chain: {:?}", e));
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(400));
            }
            Err(e) => {
                warn!(
                    request_id = %ctx.request_id,
                    signature = %sig,
                    error = %e,
                    "er_poll_error"
                );
                std::thread::sleep(Duration::from_millis(400));
            }
        }
    }
}

/// Classify an RPC error into categories
fn classify_error(error: &solana_client::client_error::ClientError) -> TxErrorDetail {
    let error_str = error.to_string();

    // Check for specific error patterns
    if error_str.contains("insufficient funds") {
        return TxErrorDetail {
            category: TxErrorCategory::InsufficientFunds,
            code: None,
            message: error_str,
            logs: vec![],
        };
    }

    if error_str.contains("blockhash not found") || error_str.contains("Blockhash not found") {
        return TxErrorDetail {
            category: TxErrorCategory::BlockhashExpired,
            code: None,
            message: error_str,
            logs: vec![],
        };
    }

    if error_str.contains("rate limit") || error_str.contains("429") {
        return TxErrorDetail {
            category: TxErrorCategory::RpcRateLimit,
            code: None,
            message: error_str,
            logs: vec![],
        };
    }

    if error_str.contains("timeout") {
        return TxErrorDetail {
            category: TxErrorCategory::RpcTimeout,
            code: None,
            message: error_str,
            logs: vec![],
        };
    }

    if error_str.contains("unavailable") || error_str.contains("connection") {
        return TxErrorDetail {
            category: TxErrorCategory::RpcUnavailable,
            code: None,
            message: error_str,
            logs: vec![],
        };
    }

    // Default to unknown
    TxErrorDetail {
        category: TxErrorCategory::Unknown,
        code: None,
        message: error_str,
        logs: vec![],
    }
}

/// Classify error from transaction status
fn classify_error_from_status(error: &solana_sdk::transaction::TransactionError) -> TxErrorDetail {
    use solana_sdk::transaction::TransactionError;

    let (category, code) = match error {
        TransactionError::InstructionError(_idx, err) => {
            // Try to extract program error code
            if let solana_sdk::instruction::InstructionError::Custom(code) = err {
                (TxErrorCategory::ProgramError(*code), Some(*code))
            } else {
                (TxErrorCategory::InvalidInstruction, None)
            }
        }
        TransactionError::InsufficientFundsForRent { .. } => {
            (TxErrorCategory::InsufficientFunds, None)
        }
        _ => (TxErrorCategory::Unknown, None),
    };

    TxErrorDetail {
        category,
        code,
        message: format!("{:?}", error),
        logs: vec![],
    }
}
