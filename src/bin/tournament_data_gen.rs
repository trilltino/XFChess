#![cfg(feature = "solana")]
//! Tournament E2E data generator — runs a real single-elimination tournament
//! on Solana devnet (default 4 players) and writes a JSON report with valid
//! explorer links for every on-chain step.
//!
//! The admin keypair must be the program's `vps_authority`
//! (keys/program-authority.json). Players are ephemeral keypairs funded from
//! the admin wallet; results are recorded by the authority (higher seed wins).
//!
//! Usage:
//!   cargo run --bin tournament_data_gen --features solana -- [--players 2|4|8|...]

use xfchess::solana::program_interface::tournament_e2e::{load_keypair, run_tournament};

const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const ADMIN_KEYPAIR: &str = "keys/program-authority.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let players = parse_players_arg().unwrap_or(4);

    println!("XFChess Tournament E2E — Solana devnet ({players} players)");
    println!("==========================================================");

    let admin = load_keypair(ADMIN_KEYPAIR)?;
    let summary = run_tournament(DEVNET_RPC, &admin, players)?;

    // JSON report with explorer links.
    let report = serde_json::json!({
        "tournament_id": summary.tournament_id,
        "player_count": summary.player_count,
        "champion": summary.champion.to_string(),
        "players": summary.players.iter()
            .map(|(name, pk)| serde_json::json!({ "name": name, "pubkey": pk.to_string() }))
            .collect::<Vec<_>>(),
        "steps": summary.steps.iter()
            .map(|s| serde_json::json!({
                "step": s.step,
                "sig": s.signature,
                "explorer": format!("https://explorer.solana.com/tx/{}?cluster=devnet", s.signature),
            }))
            .collect::<Vec<_>>(),
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });

    std::fs::create_dir_all("target")?;
    let out = "target/tournament_e2e_report.json";
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;

    println!("\nTournament {} complete.", summary.tournament_id);
    println!(
        "Champion: {} (https://explorer.solana.com/address/{}?cluster=devnet)",
        summary.champion, summary.champion
    );
    println!("Report: {out}");
    Ok(())
}

fn parse_players_arg() -> Option<u16> {
    let args: Vec<String> = std::env::args().collect();
    let idx = args.iter().position(|a| a == "--players")?;
    args.get(idx + 1)?.parse().ok()
}
