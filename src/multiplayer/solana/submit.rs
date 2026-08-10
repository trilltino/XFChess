//! Shared "build → sign → send → poll" primitive for locally-signed
//! transactions (session keys, no wallet involved).
//!
//! This is the single place that owns the send-then-poll strategy
//! (`skip_preflight`, poll interval, deadline, commitment) used by every
//! session-key-signed write path. Callers that need a wallet signature
//! still go through `tauri_signer`'s bridge; once the wallet-signed bytes
//! come back, they land here too (`submit_signed_to_rpc` is a thin
//! wrapper around `submit_and_poll`), so there is exactly one confirmation
//! strategy for all locally-submitted transactions in this codebase.

use bevy::prelude::warn;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use std::time::{Duration, Instant};

/// Tunables for `submit_and_poll`. `skip_preflight: true` matches every
/// other write path in this codebase (see the audit that introduced this
/// module) — the one place that used to skip this (`bridge.rs`'s
/// delegation tx via `send_and_confirm_transaction`) ran preflight and was
/// the slowest write path in the app for it.
#[derive(Clone, Copy, Debug)]
pub struct SubmitConfig {
    pub skip_preflight: bool,
    pub poll_interval: Duration,
    pub deadline: Duration,
    pub commitment: CommitmentConfig,
}

impl SubmitConfig {
    /// The config every existing call site used before this module existed:
    /// skip preflight, 150ms polling, confirmed commitment, 2s deadline.
    pub fn fast() -> Self {
        Self {
            skip_preflight: true,
            poll_interval: Duration::from_millis(150),
            deadline: Duration::from_secs(2),
            commitment: CommitmentConfig::confirmed(),
        }
    }
}

/// Send an already-signed transaction (legacy `Transaction` or
/// `VersionedTransaction` — anything `solana_sdk::transaction::VersionedTransaction:
/// From<T>` — via `solana_client`'s serializable-transaction bound) and poll
/// for confirmation up to `cfg.deadline`.
pub fn submit_and_poll<T>(rpc: &RpcClient, tx: &T, cfg: SubmitConfig) -> Result<Signature, String>
where
    T: solana_client::rpc_client::SerializableTransaction,
{
    let config = RpcSendTransactionConfig {
        skip_preflight: cfg.skip_preflight,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(tx, config)
        .map_err(|e| format!("send_transaction: {e}"))?;

    poll_signature(rpc, sig, cfg)
}

/// Poll an already-submitted signature for confirmation up to
/// `cfg.deadline`. Does not block past the deadline — if the deadline is
/// hit before a terminal status comes back, returns `Ok(sig)` anyway (the
/// tx was accepted by RPC; the caller isn't blocked on devnet confirmation
/// latency). A definite on-chain failure is still surfaced as `Err`.
pub fn poll_signature(
    rpc: &RpcClient,
    sig: Signature,
    cfg: SubmitConfig,
) -> Result<Signature, String> {
    let deadline = Instant::now() + cfg.deadline;
    loop {
        if Instant::now() > deadline {
            warn!(
                "[SOLANA_SUBMIT] signature {sig} accepted by RPC but not confirmed within {:?}; continuing",
                cfg.deadline
            );
            return Ok(sig);
        }
        match rpc.get_signature_status_with_commitment(&sig, cfg.commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(format!("transaction failed (sig={sig}): {e:?}")),
            Ok(None) | Err(_) => std::thread::sleep(cfg.poll_interval),
        }
    }
}

/// Fetch a fresh blockhash, build+locally-sign a legacy `Transaction` with
/// `payer` as fee-payer, and submit+poll via `submit_and_poll`. This is the
/// no-wallet counterpart to `tauri_signer::sign_and_send_via_tauri` — for
/// session-key-only writes (global session create/join, ER delegation) that
/// have no `NullSigner` slot to fill because no wallet co-signs at all.
pub fn submit_local_tx(
    rpc: &RpcClient,
    payer: &Keypair,
    instructions: &[Instruction],
    cfg: SubmitConfig,
) -> Result<Signature, String> {
    let blockhash = rpc
        .get_latest_blockhash()
        .map_err(|e| format!("get_latest_blockhash: {e}"))?;
    let tx = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    submit_and_poll(rpc, &tx, cfg)
}
