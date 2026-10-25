use xfchess_client_compat::prelude::*;
use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
use solana_system_interface::instruction as system_instruction;
use std::time::{Duration, Instant};
use tracing::{info, warn};

fn send_and_poll(
    rpc: &RpcClient,
    tx: &impl xfchess_client_compat::SdkTransaction,
    deadline: Duration,
    context: &str,
) -> Result<Signature> {
    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_sdk_transaction_with_config(tx, config)
        .map_err(|e| anyhow!(e))?;

    poll_confirmation(rpc, sig, deadline, context)
}

fn poll_confirmation(
    rpc: &RpcClient,
    sig: Signature,
    deadline: Duration,
    context: &str,
) -> Result<Signature> {
    let commitment = CommitmentConfig::confirmed();
    let end = Instant::now() + deadline;
    loop {
        if Instant::now() > end {
            return Err(anyhow!("{context} confirmation timeout for {sig}"));
        }
        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(anyhow!("{context} failed (sig={sig}): {e:?}")),
            Ok(None) => std::thread::sleep(Duration::from_millis(150)),
            Err(e) => {
                warn!("[{context}] poll error (non-fatal): {e}");
                std::thread::sleep(Duration::from_millis(150));
            }
        }
    }
}

pub fn fund_account(
    rpc: &RpcClient,
    payer: &Keypair,
    dest: &Pubkey,
    lamports: u64,
) -> Result<Signature> {
    let start = Instant::now();
    // Read-only, so a transient connection error (e.g. a pooled connection the
    // provider already closed) is retried instead of failing session hosting.
    let mut attempt = 0;
    let balance = loop {
        match rpc.get_balance(dest) {
            Ok(balance) => break balance,
            Err(e) if attempt < 2 => {
                attempt += 1;
                warn!(
                    "[SOLANA_TX] fund_account balance lookup failed (retry {attempt}/2): {}",
                    crate::signing::solana::redact_urls_in(&e.to_string())
                );
                std::thread::sleep(Duration::from_millis(200 * attempt));
            }
            Err(e) => {
                return Err(anyhow!(
                    "could not establish rent-payer balance for {dest}: {e}"
                ))
            }
        }
    };
    info!(
        "[SOLANA_TX] fund_account {dest}: balance lookup {:?}",
        start.elapsed()
    );
    let transfer_lamports = lamports.saturating_sub(balance);
    if transfer_lamports == 0 {
        info!("[SOLANA_TX] fund_account skipped for {dest}; balance already {balance} lamports");
        return Ok(Signature::default());
    }

    // The payer must cover transfer lamports, signature fees, and its own
    // rent-exempt reserve; check this before submission.
    let payer_pubkey = payer.pubkey();
    match rpc.get_balance(&payer_pubkey) {
        Ok(payer_balance) => {
            let reserve = rpc
                .get_minimum_balance_for_rent_exemption(0)
                .unwrap_or(890_880); // ~2 years rent-exempt minimum for a wallet
            const TX_FEE_LAMPORTS: u64 = 5_000;
            let needed = reserve
                .saturating_add(TX_FEE_LAMPORTS)
                .saturating_add(transfer_lamports);
            if payer_balance < needed {
                return Err(anyhow!(
                    "fee payer {payer_pubkey} is depleted: balance {payer_balance} lamports, \
                     needs >= {needed} ({transfer_lamports} transfer + {TX_FEE_LAMPORTS} fee + \
                     {reserve} rent-exempt reserve) — top up FEE_PAYER_KEYS"
                ));
            }
        }
        Err(e) => warn!("[SOLANA_TX] fund_account payer pre-flight lookup failed (non-fatal): {e}"),
    }

    let ix = system_instruction::transfer(&payer_pubkey, dest, transfer_lamports);
    info!(
        "[SOLANA_TX] fund_account {dest}: payer pre-flight done at {:?}",
        start.elapsed()
    );
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    info!(
        "[SOLANA_TX] fund_account {dest}: blockhash fetched at {:?}, submitting",
        start.elapsed()
    );

    let sig = send_and_poll(rpc, &tx, Duration::from_secs(15), "fund_account")?;
    info!(
        "[SOLANA_TX] fund_account confirmed {sig} for {dest} in {:?}",
        start.elapsed()
    );
    Ok(sig)
}

pub fn sign_and_submit(
    rpc: &RpcClient,
    signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);

    send_and_poll(rpc, &tx, Duration::from_secs(30), "sign_and_submit")
}

/// [`sign_and_submit`] on the blocking pool. Async handlers must not run the
/// blocking client inline: confirmation polling holds the thread for up to 30s.
pub async fn sign_and_submit_async(
    rpc_url: String,
    signer: std::sync::Arc<Keypair>,
    instructions: Vec<Instruction>,
) -> Result<Signature> {
    tokio::task::spawn_blocking(move || {
        sign_and_submit(&super::rpc::make_rpc(&rpc_url)?, &signer, &instructions)
    })
    .await
    .map_err(|e| anyhow!("submission task failed: {e}"))?
}

/// Like [`sign_and_submit`], but also returns the signature of the signed
/// transaction. A confirmation timeout does not mean the transaction failed;
/// callers that must not repeat or lose its effects can look the signature up
/// with [`signature_landed`] before deciding what happened.
pub fn sign_and_submit_tracked(
    rpc: &RpcClient,
    signer: &Keypair,
    instructions: &[Instruction],
) -> (Option<Signature>, Result<Signature>) {
    let blockhash = match rpc.get_latest_blockhash() {
        Ok(blockhash) => blockhash,
        Err(e) => return (None, Err(anyhow!(e))),
    };
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);
    let signature = tx.signatures.first().copied();
    let result = send_and_poll(rpc, &tx, Duration::from_secs(30), "sign_and_submit");
    (signature, result)
}

/// Whether `signature` executed successfully at `confirmed` commitment.
/// `None` means the status is unknown (not found yet, or the RPC failed).
pub fn signature_landed(rpc: &RpcClient, signature: &Signature) -> Option<bool> {
    match rpc.get_signature_status_with_commitment(signature, CommitmentConfig::confirmed()) {
        Ok(Some(Ok(()))) => Some(true),
        Ok(Some(Err(_))) => Some(false),
        Ok(None) | Err(_) => None,
    }
}

const BLOCKHASH_RETRY_ATTEMPTS: u32 = 3;

pub fn sign_and_submit_er(
    rpc: &RpcClient,
    signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    let start = Instant::now();
    for attempt in 1..=BLOCKHASH_RETRY_ATTEMPTS {
        let blockhash = rpc.get_latest_blockhash()?;
        let msg = Message::new(instructions, Some(&signer.pubkey()));
        let tx = Transaction::new(&[signer], msg, blockhash);

        match send_and_poll(rpc, &tx, Duration::from_secs(30), "ER record_move") {
            Ok(sig) => {
                tracing::info!(
                    "[ER] tx {sig} confirmed via {} in {}ms (attempt {attempt}/{BLOCKHASH_RETRY_ATTEMPTS})",
                    crate::signing::solana::redact_url(&rpc.url()),
                    start.elapsed().as_millis()
                );
                return Ok(sig);
            }
            Err(e)
                if attempt < BLOCKHASH_RETRY_ATTEMPTS
                    && e.to_string().to_lowercase().contains("blockhash not found") =>
            {
                warn!(
                    "[ER] attempt {attempt}/{BLOCKHASH_RETRY_ATTEMPTS} got 'Blockhash not found' via {} — refetching and retrying",
                    crate::signing::solana::redact_url(&rpc.url())
                );
            }
            Err(e) => return Err(e),
        }
    }
    Err(anyhow!("ER submission exhausted its retry budget"))
}

pub fn submit_signed_tx(rpc: &RpcClient, tx_bytes: &[u8]) -> Result<Signature> {
    let tx: VersionedTransaction = bincode::deserialize(tx_bytes).map_err(|e| anyhow!(e))?;
    send_and_poll(rpc, &tx, Duration::from_secs(30), "submit_signed_tx")
}

pub fn cosign_and_submit_tx(
    rpc: &RpcClient,
    session_keypair: &Keypair,
    tx_bytes: &[u8],
) -> Result<Signature> {
    let start = Instant::now();
    let mut tx: Transaction =
        bincode::deserialize(tx_bytes).map_err(|e| anyhow!("deserialize tx: {e}"))?;
    let blockhash = tx.message.recent_blockhash;
    tx.try_partial_sign(&[session_keypair], blockhash)
        .map_err(|e| anyhow!("session co-signature failed: {e}"))?;
    // Wallet-stage validation permits this key's placeholder only. Submission
    // requires the complete bundle, including a valid wallet signature.
    tx.verify()
        .map_err(|e| anyhow!("setup TX signatures invalid: {e}"))?;

    let ix_summary: Vec<String> = tx
        .message
        .instructions
        .iter()
        .enumerate()
        .map(|(i, ix)| {
            let program = tx
                .message
                .account_keys
                .get(ix.program_id_index as usize)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            format!("[{i}]={program}")
        })
        .collect();

    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_sdk_transaction_with_config(&tx, config)
        .map_err(|e| anyhow!(e))?;

    // Retain ix_summary on failure so setup transaction errors identify their instructions.
    match poll_confirmation(rpc, sig, Duration::from_secs(30), "setup TX") {
        Ok(sig) => {
            info!(
                "[SOLANA_TX] cosign_and_submit_tx confirmed {sig} in {:?}",
                start.elapsed()
            );
            Ok(sig)
        }
        Err(e) => Err(anyhow!("{e} — instructions: {}", ix_summary.join(", "))),
    }
}
