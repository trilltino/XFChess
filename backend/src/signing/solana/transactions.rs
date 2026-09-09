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
    tx: &impl solana_client::rpc_client::SerializableTransaction,
    deadline: Duration,
    context: &str,
) -> Result<Signature> {
    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(tx, config)
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
    if let Ok(balance) = rpc.get_balance(dest) {
        if balance >= lamports {
            info!(
                "[SOLANA_TX] fund_account skipped for {dest}; balance already {balance} lamports"
            );
            return Ok(Signature::default());
        }
    }

    // Pre-flight the PAYER balance before building+sending, so a depleted
    // feepayer-pool wallet fails here with an actionable message instead of
    // on-chain with the opaque `InstructionError(0, Custom(1))` (= System
    // Program ResultWithNegativeLamports). The runtime requires the payer to
    // end the tx at/above its own rent-exempt reserve AND pay the signature
    // fee, so the real floor is reserve + fee + `lamports`.
    let payer_pubkey = payer.pubkey();
    match rpc.get_balance(&payer_pubkey) {
        Ok(payer_balance) => {
            let reserve = rpc
                .get_minimum_balance_for_rent_exemption(0)
                .unwrap_or(890_880); // ~2 years rent-exempt minimum for a wallet
            const TX_FEE_LAMPORTS: u64 = 5_000;
            let needed = reserve
                .saturating_add(TX_FEE_LAMPORTS)
                .saturating_add(lamports);
            if payer_balance < needed {
                return Err(anyhow!(
                    "fee payer {payer_pubkey} is depleted: balance {payer_balance} lamports, \
                     needs >= {needed} ({lamports} transfer + {TX_FEE_LAMPORTS} fee + \
                     {reserve} rent-exempt reserve) — top up FEE_PAYER_KEYS"
                ));
            }
        }
        Err(e) => warn!("[SOLANA_TX] fund_account payer pre-flight lookup failed (non-fatal): {e}"),
    }

    let ix = system_instruction::transfer(&payer_pubkey, dest, lamports);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);

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
    unreachable!("loop always returns Ok or Err before exhausting attempts")
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
    tx.partial_sign(&[session_keypair], blockhash);

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
        .send_transaction_with_config(&tx, config)
        .map_err(|e| anyhow!(e))?;

    // Not routed through `send_and_poll`/`poll_confirmation` directly: on
    // failure this appends `ix_summary` (the instruction/program list) to
    // the error, which the shared helper's generic "{context} failed"
    // message doesn't carry — that summary is what makes setup-TX failures
    // diagnosable from logs alone.
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
