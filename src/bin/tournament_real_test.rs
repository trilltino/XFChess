#![cfg(feature = "solana")]
//! Real on-chain tournament test — runs a head-to-head (2-player) tournament
//! on Solana devnet by default, exercising the smallest bracket end-to-end:
//! initialize → escrow → shard → prize → register ×2 → start → the single
//! final match → champion recorded on-chain.
//!
//! The admin keypair must be the program's `vps_authority`
//! (keys/program-authority.json). Players are ephemeral keypairs funded from
//! the admin wallet.
//!
//! Usage:
//!   cargo run --bin tournament_real_test --features solana -- [--players 2|4|8|...]

use xfchess::solana::program_interface::tournament_e2e::{load_keypair, run_tournament};

const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const ADMIN_KEYPAIR: &str = "keys/program-authority.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let players = parse_players_arg().unwrap_or(2);
    let rpc_url = std::env::var("XFCHESS_RPC_URL").unwrap_or_else(|_| DEVNET_RPC.to_string());

    println!("XFChess Real Tournament Test — Solana devnet ({players} players)");
    println!("================================================================");

    let admin = load_keypair(ADMIN_KEYPAIR)?;
    let summary = run_tournament(&rpc_url, &admin, players)?;

    println!("\n=== PASS ===");
    println!("Tournament ID: {}", summary.tournament_id);
    println!("Champion: {}", summary.champion);
    println!("On-chain steps ({}):", summary.steps.len());
    for s in &summary.steps {
        println!(
            "  {} — https://explorer.solana.com/tx/{}?cluster=devnet",
            s.step, s.signature
        );
    }
    Ok(())
}

fn parse_players_arg() -> Option<u16> {
    let args: Vec<String> = std::env::args().collect();
    let idx = args.iter().position(|a| a == "--players")?;
    args.get(idx + 1)?.parse().ok()
}
