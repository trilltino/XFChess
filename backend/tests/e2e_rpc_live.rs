//! Tier T2 — live-network RPC smoke tests.
//!
//! `e2e_api.rs` (Tier T1) is deliberately chain-free: it points RPC URLs at an
//! unreachable port so tests stay fast, hermetic, and need no secrets. This
//! file is the opt-in counterpart — it hits whatever `SOLANA_RPC_URL` is
//! actually configured to point at (Triton One in prod/staging; public devnet
//! if you only export a bare URL locally) through the *real* client helpers in
//! `backend::signing::solana::rpc`, including the primary/fallback circuit
//! breaker.
//!
//! Every test is `#[ignore]` so a plain `cargo test` never needs network
//! access or secrets. Run this tier explicitly:
//!
//! ```text
//! just test-rpc-live
//! # or directly:
//! cargo test -p backend --test e2e_rpc_live -- --ignored --nocapture
//! ```
//!
//! after exporting `SOLANA_RPC_URL` (with your Triton x-token embedded in the
//! path, never committed) in the shell or `backend/.env`. If it isn't set,
//! each test prints a skip message and returns rather than failing — running
//! `--ignored` by accident in an environment without the secret configured
//! must stay harmless, not turn into a red build.

use backend::signing::solana::rpc::{fallback_rpc_url, make_rpc, read_with_failover, redact_url};
use std::str::FromStr;

/// The on-chain program this backend talks to (see `backend/CLAUDE.md`);
/// used as a stable, always-present account to probe read paths against.
const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

fn skip_unless_configured(test_name: &str) -> bool {
    if std::env::var("SOLANA_RPC_URL").is_ok() {
        return false;
    }
    eprintln!(
        "[skip] {test_name}: SOLANA_RPC_URL is not set — export a real RPC URL \
         (Triton devnet endpoint + x-token in prod/staging) to run this tier"
    );
    true
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn primary_rpc_reports_healthy() {
    if skip_unless_configured("primary_rpc_reports_healthy") {
        return;
    }
    let url = backend::signing::solana::rpc::rpc_url_or_devnet();
    println!("[e2e_rpc_live] probing primary RPC: {}", redact_url(&url));
    make_rpc(&url)
        .get_health()
        .expect("primary RPC should report healthy");
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn deployed_program_account_is_reachable_via_failover() {
    if skip_unless_configured("deployed_program_account_is_reachable_via_failover") {
        return;
    }
    let program_id =
        solana_sdk::pubkey::Pubkey::from_str(PROGRAM_ID).expect("valid program id constant");

    // Exercises the real primary/fallback path used in production, not just a
    // raw RpcClient call — this is what `read_with_failover` callers actually get.
    let account = read_with_failover(|rpc| rpc.get_account(&program_id))
        .expect("program account should be readable through the real failover path");
    assert!(
        account.executable,
        "program account must be marked executable on whatever cluster SOLANA_RPC_URL points at"
    );
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn fallback_url_is_independently_reachable() {
    if skip_unless_configured("fallback_url_is_independently_reachable") {
        return;
    }
    // Confirms the fallback endpoint is itself alive, so a real primary outage
    // would actually have somewhere to fail over to.
    let fb = fallback_rpc_url();
    println!("[e2e_rpc_live] probing fallback RPC: {}", redact_url(&fb));
    make_rpc(&fb)
        .get_health()
        .expect("fallback RPC should report healthy");
}
