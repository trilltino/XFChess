//! Transaction signing and submission helpers for Solana.

use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
#[allow(deprecated)]
use solana_sdk::system_instruction;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
use tracing::warn;

/// Funds `dest` with `lamports` from `payer`, submit to `rpc_url`.
pub fn fund_account(
    rpc: &RpcClient,
    payer: &Keypair,
    dest: &Pubkey,
    lamports: u64,
) -> Result<Signature> {
    let ix = system_instruction::transfer(&payer.pubkey(), dest, lamports);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    Ok(rpc.send_and_confirm_transaction(&tx)?)
}

/// Signs `ix` with `signer` (fee-payer = signer) and submits to `rpc_url`.
pub fn sign_and_submit(
    rpc: &RpcClient,
    signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(&tx, CommitmentConfig::confirmed())
        .map_err(|e| anyhow!(e))
}

/// Signs and submits to the MagicBlock ER with `skip_preflight = true`.
///
/// The ER's preflight simulator may reject transactions with -32003
/// ("Attempt to load a program that does not exist") because the XFChess
/// program is not in its preflight cache. Skipping preflight lets the TX land.
/// After sending, we poll for confirmation (ER confirms in sub-second to ~5 s).
pub fn sign_and_submit_er(
    rpc: &RpcClient,
    signer: &Keypair,
    instructions: &[Instruction],
) -> Result<Signature> {
    use solana_client::rpc_config::RpcSendTransactionConfig;
    use std::time::{Duration, Instant};

    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);

    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(&tx, config)
        .map_err(|e| anyhow!(e))?;

    let commitment = CommitmentConfig::confirmed();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err(anyhow!("ER record_move confirmation timeout for {sig}"));
        }
        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(anyhow!("ER record_move failed: {e:?}")),
            Ok(None) => std::thread::sleep(Duration::from_millis(400)),
            Err(e) => {
                warn!("[ER] poll error (non-fatal): {e}");
                std::thread::sleep(Duration::from_millis(400));
            }
        }
    }
}

/// Submits an already-signed serialized transaction.
///
/// Used for wallet-signed setup TXs. Accepts both legacy `Transaction`
/// and `VersionedTransaction` (v0). Uses `confirmed` commitment.
pub fn submit_signed_tx(rpc: &RpcClient, tx_bytes: &[u8]) -> Result<Signature> {
    let tx: VersionedTransaction = bincode::deserialize(tx_bytes).map_err(|e| anyhow!(e))?;
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(&tx, CommitmentConfig::confirmed())
        .map_err(|e| anyhow!(e))
}

/// Co-signs a wallet-signed legacy transaction with the provided session keypair,
/// then submits it.
///
/// Used by `activate_session` when `create_game` / `join_game` require both the
/// player wallet signature (already present) and the VPS session key signature.
pub fn cosign_and_submit_tx(
    rpc: &RpcClient,
    session_keypair: &Keypair,
    tx_bytes: &[u8],
) -> Result<Signature> {
    let mut tx: Transaction =
        bincode::deserialize(tx_bytes).map_err(|e| anyhow!("deserialize tx: {e}"))?;
    let blockhash = tx.message.recent_blockhash;
    tx.partial_sign(&[session_keypair], blockhash);
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(&tx, CommitmentConfig::confirmed())
        .map_err(|e| anyhow!(e))
}
