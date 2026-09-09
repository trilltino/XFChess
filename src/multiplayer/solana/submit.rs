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

#[derive(Clone, Copy, Debug)]
pub struct SubmitConfig {
    pub skip_preflight: bool,
    pub poll_interval: Duration,
    pub deadline: Duration,
    pub commitment: CommitmentConfig,
}

impl SubmitConfig {
    pub fn fast() -> Self {
        Self {
            skip_preflight: true,
            poll_interval: Duration::from_millis(150),
            deadline: Duration::from_secs(2),
            commitment: CommitmentConfig::confirmed(),
        }
    }
}

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
