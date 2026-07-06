//! Transaction-landing test: submit small SPL-Memo transactions and time both the
//! `sendTransaction` round-trip and the time-to-confirmation, per endpoint.
//!
//! This validates the "better transaction landing" claim. Memo txs touch no accounts
//! and only cost the base fee, so a funded master keypair with a fraction of a SOL
//! covers a full run.

use std::str::FromStr;
use std::time::{Duration, Instant};

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use super::{redact_url, LatencyStats};

/// SPL Memo program (v2). Accepts zero accounts, so a memo tx is the cheapest
/// possible "real" transaction to land.
const MEMO_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

/// Outcome of a tx-landing run against one endpoint.
pub struct TxLandReport {
    pub name: String,
    pub submit: LatencyStats,
    pub confirm: LatencyStats,
    pub landed: usize,
    pub failed: usize,
}

/// Submit `count` memo transactions through `url`, signed by `payer`, measuring
/// submit latency and confirmation latency for each.
///
/// Blocking — call inside `tokio::task::spawn_blocking`.
pub fn run(name: &str, url: &str, payer: &Keypair, count: usize) -> TxLandReport {
    let rpc = RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed());
    let memo = Pubkey::from_str(MEMO_PROGRAM_ID).expect("valid memo program id");

    let mut report = TxLandReport {
        name: name.to_string(),
        submit: LatencyStats::default(),
        confirm: LatencyStats::default(),
        landed: 0,
        failed: 0,
    };

    println!("\n── {name} ──  {}", redact_url(url));

    for i in 0..count {
        let nonce = format!("triton-bench {i} {}", unique_nanos());
        let ix = Instruction {
            program_id: memo,
            accounts: vec![],
            data: nonce.into_bytes(),
        };

        let blockhash = match rpc.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => {
                eprintln!("   tx {i}: blockhash fetch failed: {e}");
                report.failed += 1;
                continue;
            }
        };

        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);

        let submit_start = Instant::now();
        let sig = match rpc.send_transaction(&tx) {
            Ok(sig) => {
                report
                    .submit
                    .record_ms(submit_start.elapsed().as_secs_f64() * 1000.0);
                sig
            }
            Err(e) => {
                eprintln!("   tx {i}: send failed: {e}");
                report.failed += 1;
                continue;
            }
        };

        // Poll for confirmation, timing how long the endpoint takes to reflect it.
        let confirm_start = Instant::now();
        let mut confirmed = false;
        while confirm_start.elapsed() < Duration::from_secs(45) {
            if rpc.confirm_transaction(&sig).unwrap_or(false) {
                confirmed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }

        if confirmed {
            report
                .confirm
                .record_ms(confirm_start.elapsed().as_secs_f64() * 1000.0);
            report.landed += 1;
            println!(
                "   tx {:>2}/{count}: landed in {:>6.0} ms  ({sig})",
                i + 1,
                confirm_start.elapsed().as_secs_f64() * 1000.0
            );
        } else {
            report.failed += 1;
            println!("   tx {:>2}/{count}: NOT confirmed in 45s  ({sig})", i + 1);
        }
    }

    report
}

/// Print a side-by-side summary of multiple endpoint reports.
pub fn print_summary(reports: &[TxLandReport]) {
    println!("\n── TX LANDING SUMMARY ──");
    println!(
        "   {:>16} │ {:>7} │ {:>11} │ {:>12} │ {:>12}",
        "endpoint", "landed", "submit p50", "confirm p50", "confirm p95"
    );
    println!("   {}", "─".repeat(72));
    for r in reports {
        println!(
            "   {:>16} │ {:>3}/{:<3} │ {:>8.0} ms │ {:>9.0} ms │ {:>9.0} ms",
            truncate(&r.name, 16),
            r.landed,
            r.landed + r.failed,
            r.submit.percentile(50.0),
            r.confirm.percentile(50.0),
            r.confirm.percentile(95.0),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}

fn unique_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
