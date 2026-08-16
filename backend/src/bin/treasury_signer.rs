//! Standalone treasury-withdrawal signer — deliberately NOT part of
//! `signing-server`.
//!
//! Phase 2 of the Custody Ledger hardening plan calls for isolating
//! `treasury_authority` off the general-purpose, always-on, internet-facing
//! signing host: a compromise of that process (or of any of the many other
//! authorities it holds) must not also be able to drain the treasury. This
//! binary is the isolation boundary — it is the *only* place
//! `TREASURY_AUTHORITY_KEY` is ever loaded, it never binds a network port,
//! and it's meant to be run interactively by an operator on a separate,
//! minimally-networked host (or the same host, but never as a spawned child
//! of `signing-server`).
//!
//! `signing-server`'s `AppState` no longer holds a treasury signing key at
//! all — only the public key (`treasury_authority_pubkey` in
//! `SigningConfig`, not a secret, matches the hardcoded on-chain constant).
//! `POST /admin/tournament/{id}/...` and the treasury-refund admin route
//! log the requested withdrawal and hand the operator the exact command to
//! run here; the networked process never touches the private key.
//!
//! ```text
//! TREASURY_AUTHORITY_KEY=<base58-or-keyfile-path> \
//! cargo run --bin treasury_signer -- <destination-pubkey> <lamports> [reason]
//! ```
//!
//! Optional env overrides: `SOLANA_RPC_URL` (default devnet),
//! `PROGRAM_ID` (default the deployed devnet/mainnet program id).

use backend::signing::load_keypair_from_env_value;
use backend::signing::solana::{make_rpc, sign_and_submit, withdraw_treasury_ix};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::io::{self, Write};
use std::str::FromStr;

const DEFAULT_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";
const DEFAULT_RPC_URL: &str = "https://api.devnet.solana.com";

fn usage() -> ! {
    eprintln!(
        "usage: treasury_signer <destination-pubkey> <lamports> [reason]\n\n\
         Requires TREASURY_AUTHORITY_KEY (base58 secret key or a JSON keyfile \
         path) in the environment. Never run this on the same host/process as \
         signing-server — see this binary's module doc for why."
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 {
        usage();
    }

    let destination = match Pubkey::from_str(&args[0]) {
        Ok(pk) => pk,
        Err(e) => {
            eprintln!("[treasury_signer] invalid destination pubkey '{}': {e}", args[0]);
            std::process::exit(1);
        }
    };
    let lamports: u64 = match args[1].parse() {
        Ok(n) if n > 0 => n,
        _ => {
            eprintln!("[treasury_signer] lamports must be a positive integer, got '{}'", args[1]);
            std::process::exit(1);
        }
    };
    let reason = args.get(2).map(String::as_str).unwrap_or("(no reason given)");

    let key_val = std::env::var("TREASURY_AUTHORITY_KEY").unwrap_or_else(|_| {
        eprintln!("[treasury_signer] TREASURY_AUTHORITY_KEY is not set — refusing to run");
        std::process::exit(1);
    });
    let authority = load_keypair_from_env_value(&key_val).unwrap_or_else(|e| {
        eprintln!("[treasury_signer] TREASURY_AUTHORITY_KEY is set but invalid: {e}");
        std::process::exit(1);
    });

    let program_id_str =
        std::env::var("PROGRAM_ID").unwrap_or_else(|_| DEFAULT_PROGRAM_ID.to_string());
    let program_id = Pubkey::from_str(&program_id_str).unwrap_or_else(|e| {
        eprintln!("[treasury_signer] invalid PROGRAM_ID '{program_id_str}': {e}");
        std::process::exit(1);
    });
    let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());

    println!("=== Treasury Withdrawal ===");
    println!("  Authority:   {}", authority.pubkey());
    println!("  Destination: {destination}");
    println!(
        "  Amount:      {lamports} lamports ({:.9} SOL)",
        lamports as f64 / 1_000_000_000.0
    );
    println!("  Reason:      {reason}");
    println!("  RPC:         {rpc_url}");
    println!("  Program:     {program_id}");
    print!("\nThis is irreversible. Type CONFIRM to proceed: ");
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    if input.trim() != "CONFIRM" {
        println!("Aborted — no transaction submitted.");
        std::process::exit(1);
    }

    let rpc = make_rpc(&rpc_url);
    let ix = withdraw_treasury_ix(&program_id, &authority.pubkey(), &destination, lamports);
    match sign_and_submit(&rpc, &authority, &[ix]) {
        Ok(sig) => {
            println!("\n[treasury_signer] SUCCESS — signature: {sig}");
        }
        Err(e) => {
            eprintln!("\n[treasury_signer] FAILED: {e}");
            std::process::exit(1);
        }
    }
}
