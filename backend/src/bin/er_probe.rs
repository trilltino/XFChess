//! Diagnostic: which endpoint's blockhash does the ER actually accept?
//!
//! Every ER-routed instruction (`record_move`, `undelegate_game`,
//! `schedule_time_check`) has been failing with
//! `-32003 ... Blockhash not found`. `sign_and_submit_er` fetches a blockhash
//! and sends the transaction to the *same* URL, so the usual "stale blockhash"
//! explanation doesn't apply — unless the URL's read path and write path are
//! backed by different chains.
//!
//! This probe sends a deliberately-doomed transfer (random unfunded keypair,
//! 0 lamports) to each endpoint using that endpoint's own blockhash. The
//! transaction can never succeed — that's the point. What matters is *which
//! error* comes back:
//!
//!   - "Blockhash not found"        -> the endpoint rejected the blockhash it
//!                                     itself just handed us: read and write
//!                                     paths disagree. Unusable for ER writes.
//!   - "Attempt to debit ..." /
//!     "AccountNotFound" / similar  -> blockhash ACCEPTED; the tx got far
//!                                     enough to fail on funding. This is the
//!                                     endpoint we should be submitting to.
//!
//! Run: `cargo run --bin er_probe`

use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

fn probe(label: &str, url: &str) {
    println!("\n=== {label} — {url}");
    let rpc = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    let blockhash = match rpc.get_latest_blockhash() {
        Ok(b) => b,
        Err(e) => {
            println!("  getLatestBlockhash FAILED: {e}");
            return;
        }
    };
    println!("  blockhash fetched: {blockhash}");

    // Random, unfunded, 0-lamport self-transfer. Guaranteed to fail on funds,
    // never on anything that could mutate real state.
    let kp = Keypair::new();
    let ix = system_instruction::transfer(&kp.pubkey(), &kp.pubkey(), 0);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&kp.pubkey()), &[&kp], blockhash);

    // skip_preflight so the *cluster* judges the blockhash, not a simulator.
    let cfg = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    match rpc.send_transaction_with_config(&tx, cfg) {
        Ok(sig) => println!("  sendTransaction accepted (sig {sig}) — blockhash OK"),
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("blockhash not found") {
                println!("  >>> BLOCKHASH REJECTED BY ITS OWN ENDPOINT <<<");
            } else {
                println!("  blockhash ACCEPTED (failed later, as designed)");
            }
            println!("  raw: {msg}");
        }
    }
}

/// The decisive test. A transaction that *touches the delegated Game PDA*
/// forces the router to forward it to whichever ER validator owns that
/// delegation — unlike the plain transfer above, which the router can settle
/// on its own chain. If the router hands out a blockhash from its own chain
/// but forwards the transaction to a different validator's chain, this is
/// exactly where "Blockhash not found" appears.
///
/// The instruction data is a garbage discriminator, so the program rejects it
/// — again, deliberately. Reaching a *program* error means the blockhash and
/// routing were fine and only the payload was bad, which is the success
/// condition for this probe.
fn probe_delegated(label: &str, url: &str, program_id: &str, game_pda: &str) {
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    println!("\n=== {label} (touching delegated PDA) — {url}");
    let rpc = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());

    let blockhash = match rpc.get_latest_blockhash() {
        Ok(b) => b,
        Err(e) => {
            println!("  getLatestBlockhash FAILED: {e}");
            return;
        }
    };
    println!("  blockhash fetched: {blockhash}");

    let program = Pubkey::from_str(program_id).expect("program id");
    let pda = Pubkey::from_str(game_pda).expect("game pda");
    let kp = Keypair::new();

    let ix = Instruction::new_with_bytes(
        program,
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF], // bogus discriminator
        vec![
            AccountMeta::new(pda, false),
            AccountMeta::new(kp.pubkey(), true),
        ],
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&kp.pubkey()), &[&kp], blockhash);

    let cfg = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    match rpc.send_transaction_with_config(&tx, cfg) {
        Ok(sig) => println!("  accepted (sig {sig}) — blockhash + routing OK"),
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("blockhash not found") {
                println!("  >>> BLOCKHASH NOT FOUND — this is the production failure <<<");
            } else {
                println!("  blockhash + routing OK (rejected later, as designed)");
            }
            println!("  raw: {msg}");
        }
    }
}

fn main() {
    let router = std::env::var("MAGIC_ROUTER_RPC_URL")
        .unwrap_or_else(|_| "https://devnet-router.magicblock.app".into());
    let er =
        std::env::var("ER_RPC_URL").unwrap_or_else(|_| "https://devnet-eu.magicblock.app/".into());
    let program = std::env::var("PROGRAM_ID")
        .unwrap_or_else(|_| "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into());
    // Game 16792328953888037348 from the 15:42 live run — delegated and never
    // undelegated (its undelegate hit this same bug), so it is still ER-owned.
    let game_pda = std::env::var("GAME_PDA")
        .unwrap_or_else(|_| "3a3wnHhVT27GtdvGpshJLDUPAfVDdf4m8PDiL1njA8Qp".into());

    println!("Probing ER write paths (doomed txs, no state change)");
    probe("MAGIC ROUTER", &router);
    probe("ER VALIDATOR", &er);

    println!("\n---- with a delegated account in the tx (the real routing case) ----");
    probe_delegated("MAGIC ROUTER", &router, &program, &game_pda);
    probe_delegated("ER VALIDATOR", &er, &program, &game_pda);
}
