    //! admin_tournament — CLI for tournament lifecycle management.
//!
//! Uses the admin keypair directly (no Phantom popup).
//! Calls on-chain instructions AND the local signing-server VPS.
//!
//! Build:  cargo build --features solana --bin admin_tournament
//! Usage:  target\debug\admin_tournament.exe <COMMAND> [OPTIONS]

use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};

#[allow(deprecated)]
use solana_sdk::system_program;
// ── Constants ─────────────────────────────────────────────────────────────────

const PROGRAM_ID: &str = "C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf";
const TOURNAMENT_SEED: &[u8] = b"tournament";
const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";
const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";

// ── Main ──────────────────────────────────────────────────────────────────────

pub fn run(action: &super::TournamentCommand, rpc_url: &str, vps_url: &str, keypair_path: &str) {
    let keypair = read_keypair_file(keypair_path).unwrap_or_else(|e| {
        eprintln!("[ERROR] Cannot read keypair {}: {}", keypair_path, e);
        std::process::exit(1);
    });

    let program_id: Pubkey = PROGRAM_ID.parse().expect("invalid program ID");
    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    dispatch(&rpc, &keypair, program_id, vps_url, action);
}

fn dispatch(rpc: &RpcClient, authority: &Keypair, program_id: Pubkey, vps: &str, action: &super::TournamentCommand) {
    match action {
        super::TournamentCommand::Create { name, entry_fee, base_time_seconds, increment_seconds, password } => {
            cmd_create(rpc, authority, program_id, vps, name, *entry_fee, *base_time_seconds, *increment_seconds, password.clone());
        }
        super::TournamentCommand::List => {
            println!("[LIST] Active tournaments — query the VPS at {}/tournaments", vps);
        }
        super::TournamentCommand::Info { id } => {
            cmd_status(rpc, program_id, *id);
        }
        super::TournamentCommand::Start { id } => {
            cmd_start(rpc, authority, program_id, vps, *id);
        }
        super::TournamentCommand::Record { id, match_index, winner } => {
            let winner_pubkey = winner.parse().expect("Invalid winner pubkey");
            cmd_record(rpc, authority, program_id, vps, *id, *match_index, winner_pubkey);
        }
        super::TournamentCommand::Advance { id } => {
            cmd_advance(rpc, authority, program_id, *id);
        }
        super::TournamentCommand::Status { id } => {
            cmd_status(rpc, program_id, *id);
        }
        super::TournamentCommand::TestFill { id } => {
            cmd_test_fill(rpc, authority, program_id, *id);
        }
        super::TournamentCommand::Cancel { id } => {
            println!("[CANCEL] Tournament {} — implement cancel logic", id);
        }
        super::TournamentCommand::SetPassword { id, password } => {
            cmd_set_password(vps, *id, password.clone());
        }
    }
}

// ── Command handlers ──────────────────────────────────────────────────────────

fn cmd_create(
    rpc: &RpcClient,
    authority: &Keypair,
    program_id: Pubkey,
    vps: &str,
    name: &str,
    entry_fee_sol: f64,
    base_time_seconds: u64,
    increment_seconds: u16,
    password: Option<String>,
) {
    let entry_fee_lamports = (entry_fee_sol * 1_000_000_000.0) as u64;
    let tournament_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 1_000_000;

    println!("[CREATE] Tournament \"{}\"  ID={}  fee={} SOL  time={}+{}s",
        name, tournament_id, entry_fee_sol, base_time_seconds, increment_seconds);
    if password.is_some() {
        println!("[CREATE] Tournament is private with password set");
    }

    let ix = initialize_tournament_ix(program_id, authority.pubkey(), tournament_id, name, entry_fee_lamports, base_time_seconds, increment_seconds);
    send_and_confirm(rpc, authority, &[ix], "initialize_tournament");

    println!("\n[VPS] Registering tournament with signing server...");
    let client = reqwest::blocking::Client::new();
    let mut body = serde_json::json!({
        "tournament_id": tournament_id,
        "name": name,
        "entry_fee_lamports": entry_fee_lamports
    });
    if let Some(pw) = password {
        body["password"] = serde_json::Value::String(pw);
    }
    match client.post(format!("{}/admin/tournament/create", vps)).json(&body).send() {
        Ok(r) => println!("[VPS] {}", r.status()),
        Err(e) => eprintln!("[VPS] Warning: {}", e),
    }

    println!("\n════════════════════════════════════════");
    println!("  TOURNAMENT ID:  {}", tournament_id);
    println!("  Share this ID with players.");
    println!("  Players join via the Tournaments screen.");
    println!("════════════════════════════════════════");
}

fn cmd_start(rpc: &RpcClient, authority: &Keypair, program_id: Pubkey, vps: &str, id: u64) {
    println!("[START] Starting bracket for tournament {}...", id);
    let ix = start_tournament_ix(program_id, authority.pubkey(), id);
    send_and_confirm(rpc, authority, &[ix], "start_tournament");

    let client = reqwest::blocking::Client::new();
    let _ = client.post(format!("{}/admin/tournament/{}/start", vps, id)).send();

    println!("[OK] Bracket started. Players will see their match assignments in-game.");
}

fn cmd_record(
    rpc: &RpcClient,
    authority: &Keypair,
    program_id: Pubkey,
    vps: &str,
    id: u64,
    match_index: u8,
    winner: Pubkey,
) {
    let label = ["SF1", "SF2", "Final"].get(match_index as usize).unwrap_or(&"Match");
    println!("[RECORD] Tournament {} — {} winner: {}", id, label, winner);
    let ix = record_match_result_ix(program_id, authority.pubkey(), id, match_index, winner);
    send_and_confirm(rpc, authority, &[ix], "record_match_result");

    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({ "match_index": match_index, "winner": winner.to_string() });
    let _ = client.post(format!("{}/admin/tournament/{}/record-result", vps, id)).json(&body).send();

    println!("[OK] {} result recorded.", label);
    if match_index == 1 {
        println!("\nBoth semi-finals done. Run:");
        println!("  admin_tournament.exe advance --id {}", id);
    }
    if match_index == 2 {
        println!("\nTournament complete! Winner can now claim prize in-game.");
    }
}

fn cmd_advance(rpc: &RpcClient, authority: &Keypair, program_id: Pubkey, id: u64) {
    println!("[ADVANCE] Setting up final for tournament {}...", id);
    let ix = advance_final_ix(program_id, authority.pubkey(), id);
    send_and_confirm(rpc, authority, &[ix], "advance_final");
    println!("[OK] Final match configured. Players will see their assignments in-game.");
}

fn cmd_test_fill(rpc: &RpcClient, _admin: &Keypair, program_id: Pubkey, id: u64) {
    use solana_sdk::native_token::LAMPORTS_PER_SOL;

    // Fetch entry fee from tournament PDA data (offset 8+8+8+4+name_len+8 = variable).
    // Simpler: just airdrop 0.5 SOL per player which covers any reasonable entry fee + rent.
    let airdrop_amount = LAMPORTS_PER_SOL / 2; // 0.5 SOL each

    println!("[TEST-FILL] Generating 4 test wallets and registering for tournament {}...", id);

    let profile_seed: &[u8] = b"profile";
    let tournament_pda_key = tournament_pda(program_id, id);
    let escrow_key = escrow_pda(program_id, id);

    for i in 0..4usize {
        let player = Keypair::new();
        println!("  Player {}: {}", i, player.pubkey());

        // Airdrop
        match rpc.request_airdrop(&player.pubkey(), airdrop_amount) {
            Ok(sig) => {
                let _ = rpc.confirm_transaction(&sig);
                println!("    Airdrop confirmed");
            }
            Err(e) => {
                eprintln!("    [WARN] Airdrop failed (rate limit?): {} — trying to continue", e);
            }
        }

        // init_profile
        let profile_pda = Pubkey::find_program_address(
            &[profile_seed, player.pubkey().as_ref()], &program_id,
        ).0;
        let init_data = discriminator("init_profile").to_vec();
        let init_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(profile_pda, false),
                AccountMeta::new(player.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: init_data,
        };

        // register_player
        let mut reg_data = discriminator("register_player").to_vec();
        reg_data.extend_from_slice(&id.to_le_bytes());
        let reg_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(tournament_pda_key, false),
                AccountMeta::new(escrow_key, false),
                AccountMeta::new_readonly(profile_pda, false),
                AccountMeta::new(player.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: reg_data,
        };

        // Send both in one tx signed by the player
        let blockhash = match rpc.get_latest_blockhash() {
            Ok(bh) => bh,
            Err(e) => { eprintln!("    [ERROR] get_latest_blockhash: {}", e); continue; }
        };
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, reg_ix],
            Some(&player.pubkey()),
            &[&player],
            blockhash,
        );
        match rpc.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(sig) => println!("    Registered: {}", sig),
            Err(e) => eprintln!("    [ERROR] register player {}: {}", i, e),
        }
    }

    println!("\n[TEST-FILL] Done. Now run:");
    println!("  scripts\\admin_tournament.bat start --id {}", id);
}

fn cmd_status(rpc: &RpcClient, program_id: Pubkey, id: u64) {
    let tournament_pda = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &id.to_le_bytes()], &program_id,
    ).0;
    match rpc.get_account(&tournament_pda) {
        Ok(acct) => {
            println!("[STATUS] Tournament {} — PDA: {}", id, tournament_pda);
            println!("  Account size: {} bytes, lamports: {}", acct.data.len(), acct.lamports);
        }
        Err(e) => {
            println!("[STATUS] Tournament {} not found on-chain: {}", id, e);
        }
    }
}

fn cmd_set_password(vps: &str, tournament_id: u64, password: String) {
    println!("[SET_PASSWORD] Setting password for tournament ID={}", tournament_id);
    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "password": password
    });
    match client.post(format!("{}/admin/tournament/{}/set-password", vps, tournament_id)).json(&body).send() {
        Ok(r) => println!("[VPS] {}", r.status()),
        Err(e) => eprintln!("[VPS] Error: {}", e),
    }
    println!("[SET_PASSWORD] Password updated for tournament {}", tournament_id);
}

// ── Transaction helper ────────────────────────────────────────────────────────

fn send_and_confirm(rpc: &RpcClient, signer: &Keypair, ixs: &[Instruction], label: &str) {
    let recent_blockhash = rpc.get_latest_blockhash().unwrap_or_else(|e| {
        eprintln!("[ERROR] get_latest_blockhash: {}", e);
        std::process::exit(1);
    });
    let tx = Transaction::new_signed_with_payer(ixs, Some(&signer.pubkey()), &[signer], recent_blockhash);
    match rpc.send_and_confirm_transaction_with_spinner(&tx) {
        Ok(sig) => println!("[OK] {} confirmed: {}", label, sig),
        Err(e) => {
            eprintln!("[ERROR] {} failed: {}", label, e);
            std::process::exit(1);
        }
    }
}

// ── Instruction builders (inline, no crate dependency) ───────────────────────

fn discriminator(fn_name: &str) -> [u8; 8] {
    let mut h = Sha256::new();
    h.update(format!("global:{}", fn_name).as_bytes());
    let hash = h.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + s.len());
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf
}

fn tournament_pda(program_id: Pubkey, id: u64) -> Pubkey {
    Pubkey::find_program_address(&[TOURNAMENT_SEED, &id.to_le_bytes()], &program_id).0
}
fn escrow_pda(program_id: Pubkey, id: u64) -> Pubkey {
    Pubkey::find_program_address(&[TOURNAMENT_ESCROW_SEED, &id.to_le_bytes()], &program_id).0
}
fn match_pda(program_id: Pubkey, id: u64, index: u8) -> Pubkey {
    Pubkey::find_program_address(&[TOURNAMENT_MATCH_SEED, &id.to_le_bytes(), &[index]], &program_id).0
}

fn initialize_tournament_ix(program_id: Pubkey, authority: Pubkey, id: u64, name: &str, entry_fee: u64, base_time_seconds: u64, increment_seconds: u16) -> Instruction {
    let mut data = discriminator("initialize_tournament").to_vec();
    data.extend_from_slice(&id.to_le_bytes());
    data.extend(borsh_string(name));
    data.extend_from_slice(&entry_fee.to_le_bytes());
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda(program_id, id), false),
            AccountMeta::new(escrow_pda(program_id, id), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn start_tournament_ix(program_id: Pubkey, authority: Pubkey, id: u64) -> Instruction {
    let mut data = discriminator("start_tournament").to_vec();
    data.extend_from_slice(&id.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda(program_id, id), false),
            AccountMeta::new(match_pda(program_id, id, 0), false),
            AccountMeta::new(match_pda(program_id, id, 1), false),
            AccountMeta::new(match_pda(program_id, id, 2), false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn record_match_result_ix(program_id: Pubkey, authority: Pubkey, id: u64, match_index: u8, winner: Pubkey) -> Instruction {
    let mut data = discriminator("record_match_result").to_vec();
    data.extend_from_slice(&id.to_le_bytes());
    data.push(match_index);
    data.extend_from_slice(winner.as_ref());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda(program_id, id), false),
            AccountMeta::new(match_pda(program_id, id, match_index), false),
            AccountMeta::new_readonly(Pubkey::default(), false), // Placeholder for game PDA (Unchecked)
            AccountMeta::new(authority, true),
        ],
        data,
    }
}

fn advance_final_ix(program_id: Pubkey, authority: Pubkey, id: u64) -> Instruction {
    let mut data = discriminator("advance_final").to_vec();
    data.extend_from_slice(&id.to_le_bytes());
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tournament_pda(program_id, id), false),
            AccountMeta::new_readonly(match_pda(program_id, id, 0), false),
            AccountMeta::new_readonly(match_pda(program_id, id, 1), false),
            AccountMeta::new(match_pda(program_id, id, 2), false),
            AccountMeta::new(authority, true),
        ],
        data,
    }
}
