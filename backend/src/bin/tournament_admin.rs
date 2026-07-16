//! XFChess Tournament Admin CLI
//!
//! A dynamic admin tool for creating and managing tournaments with full
//! control over scheduling, entrant counts, KYC/CACF gating, format, and prizes.
//!
//! # Usage
//! ```
//! cargo run --bin tournament_admin
//! ```
//!
//! # Required environment variables
//! - `ADMIN_API_KEY`  — admin API key for the signing server
//!
//! # Optional environment variables
//! - `SIGNING_SERVICE_URL` — server base URL (default: local HTTP server)
//!
//! # KYC / CACF
//! When `kyc_required = true` is set on a tournament, the server already stores
//! a `kyc_required` flag on `TournamentRecord`. This CLI also exposes a live
//! KYC status check per wallet so you can audit entrants before opening play.
//!
//! Reference: `backend/src/signing/routes/identity.rs` — `GET /identity/status/{pubkey}`

use std::env;
use std::io::{self, Write};

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

// ── Config ────────────────────────────────────────────────────────────────────

static SERVICE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn server_url() -> String {
    SERVICE_URL.get_or_init(resolve_service_url).clone()
}

/// Resolve the backend URL once. An explicit `SIGNING_SERVICE_URL` wins (but is
/// rejected if it is plain http to a non-loopback host). With no env set, prompt
/// the operator to pick LOCAL or PRODUCTION (which needs an SSH tunnel).
fn resolve_service_url() -> String {
    if let Ok(url) = env::var("SIGNING_SERVICE_URL") {
        if is_insecure_remote(&url) {
            panic!(
                "SIGNING_SERVICE_URL={url} is plain http to a non-loopback host. \
                 Use https, or a loopback SSH-tunnel port (http://127.0.0.1:8091)."
            );
        }
        return url;
    }

    println!("\nSelect environment:");
    println!("  1. LOCAL      http://127.0.0.1:8090");
    println!("  2. PRODUCTION http://127.0.0.1:8091  (requires an SSH tunnel)");
    print!("  Choice [1/2]: ");
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();

    match buf.trim() {
        "2" => {
            let url = "http://127.0.0.1:8091".to_string();
            if !health_ok(&url) {
                eprintln!("\n[!] No backend reachable at {url}. Open the tunnel first:");
                eprintln!(
                    "    ssh -i ~/.ssh/xfchess_vps -N -L 8091:127.0.0.1:8090 tunnel@178.104.55.19\n"
                );
                std::process::exit(1);
            }
            url
        }
        _ => "http://127.0.0.1:8090".to_string(),
    }
}

/// True for an `http://` URL whose host is neither 127.0.0.1 nor localhost.
fn is_insecure_remote(url: &str) -> bool {
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or("");
        return host != "127.0.0.1" && host != "localhost";
    }
    false
}

/// Blocking GET {url}/health with a short timeout; true on 2xx.
fn health_ok(url: &str) -> bool {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()
        .and_then(|c| c.get(format!("{url}/health")).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn api_key() -> String {
    env::var("ADMIN_API_KEY").expect("ADMIN_API_KEY must be set")
}

fn client() -> reqwest::blocking::Client {
    let key = api_key();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Content-Type",
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "X-API-Key",
        reqwest::header::HeaderValue::from_str(&key)
            .expect("ADMIN_API_KEY contains invalid HTTP header characters"),
    );
    reqwest::blocking::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("Failed to build HTTP client")
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

fn prompt(label: &str) -> String {
    print!("  {}: ", label);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().to_string()
}

fn prompt_default(label: &str, default: &str) -> String {
    print!("  {} [{}]: ", label, default);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    let s = buf.trim().to_string();
    if s.is_empty() {
        default.to_string()
    } else {
        s
    }
}

fn confirm(label: &str) -> bool {
    let s = prompt_default(label, "N").to_lowercase();
    s == "y" || s == "yes"
}

fn read_u64(label: &str) -> u64 {
    loop {
        match prompt(label).parse::<u64>() {
            Ok(n) => return n,
            Err(_) => println!("  [!] Not a valid number, try again."),
        }
    }
}

fn read_u16(label: &str) -> u16 {
    loop {
        match prompt(label).parse::<u16>() {
            Ok(n) => return n,
            Err(_) => println!("  [!] Not a valid number, try again."),
        }
    }
}

fn read_usize(label: &str) -> usize {
    loop {
        match prompt(label).parse::<usize>() {
            Ok(n) => return n,
            Err(_) => println!("  [!] Not a valid number, try again."),
        }
    }
}

fn read_u32_opt(label: &str) -> Option<u32> {
    let s = prompt(&format!("{} (leave blank to skip)", label));
    if s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// Parse a human-readable UTC datetime string into a Unix timestamp.
/// Accepted format: "YYYY-MM-DD HH:MM" or "YYYY-MM-DD HH:MM:SS"
fn parse_datetime(s: &str) -> Option<i64> {
    let fmts = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"];
    for fmt in &fmts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(Utc.from_utc_datetime(&naive).timestamp());
        }
    }
    None
}

fn read_scheduled_at() -> Option<i64> {
    println!("  Enter scheduled start time in UTC (format: YYYY-MM-DD HH:MM)");
    println!("  Leave blank to open tournament immediately.");
    let s = prompt("Scheduled at (UTC)");
    if s.is_empty() {
        return None;
    }
    match parse_datetime(&s) {
        Some(ts) => {
            let dt: DateTime<Utc> = DateTime::from_timestamp(ts, 0).unwrap_or_default();
            println!(
                "  [OK] Scheduled for: {} UTC (Unix: {})",
                dt.format("%Y-%m-%d %H:%M:%S"),
                ts
            );
            Some(ts)
        }
        None => {
            println!("  [!] Could not parse datetime — tournament will open immediately.");
            None
        }
    }
}

fn read_max_players() -> u16 {
    println!("  For Single Elimination: must be a power of 2 (2, 4, 8, 16, 32, 64, 128, 256)");
    println!("  For Swiss: any value >= 2");
    loop {
        let n = read_u16("Max entrants");
        if n >= 2 {
            return n;
        }
        println!("  [!] Minimum 2 players required.");
    }
}

fn read_prize_shares(max_players: u16, is_free: bool) -> [u16; 4] {
    if is_free {
        println!("  [FREE] No monetary prizes.");
        return [0, 0, 0, 0];
    }
    let default_shares = if max_players >= 16 {
        [5000u16, 3000, 1500, 500]
    } else {
        [10000u16, 0, 0, 0]
    };
    let default_label = format!(
        "{:.0}% / {:.0}% / {:.0}% / {:.0}%",
        default_shares[0] as f32 / 100.0,
        default_shares[1] as f32 / 100.0,
        default_shares[2] as f32 / 100.0,
        default_shares[3] as f32 / 100.0,
    );
    println!("  Default prize split: {}", default_label);
    if !confirm("Customise prize shares?") {
        return default_shares;
    }
    println!("  Enter shares as basis points (10000 = 100%). Must sum to ≤ 10000.");
    let first = read_u16("1st place (bps)");
    let second = read_u16("2nd place (bps)");
    let third = read_u16("3rd place (bps)");
    let fourth = read_u16("4th place (bps)");
    let total = first as u32 + second as u32 + third as u32 + fourth as u32;
    if total > 10000 {
        println!("  [!] Total {} bps > 10000. Using defaults.", total);
        return default_shares;
    }
    [first, second, third, fourth]
}

// ── API types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TournamentSummary {
    tournament_id: u64,
    name: String,
    entry_fee_lamports: u64,
    max_players: u16,
    registered: usize,
    status: String,
}

#[derive(Debug, Deserialize)]
struct TournamentDetail {
    tournament_id: u64,
    name: String,
    status: String,
    max_players: u16,
    entry_fee_lamports: u64,
    players: Vec<String>,
    player_elos: Option<Vec<u32>>,
    prize_pool: Option<u64>,
    prize_shares: [u16; 4],
    winner: Option<String>,
    second_place: Option<String>,
    third_place: Option<String>,
    fourth_place: Option<String>,
    kyc_required: Option<bool>,
    scheduled_at: Option<i64>,
    elo_min: Option<u32>,
    elo_max: Option<u32>,
}

#[derive(Debug, Serialize)]
struct CreateTournamentReq {
    tournament_id: u64,
    name: String,
    entry_fee_lamports: u64,
    max_players: u16,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    swiss_rounds: Option<u8>,
    prize_shares: Option<[u16; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elo_min: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elo_max: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_players: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_at: Option<i64>,
    kyc_required: bool,
}

#[derive(Debug, Serialize)]
struct RecordResultReq {
    match_index: usize,
    winner: String,
    loser: String,
}

#[derive(Debug, Serialize)]
struct SetMatchGameIdReq {
    match_index: usize,
    game_id: u64,
}

#[derive(Debug, Deserialize)]
struct KycStatus {
    verified: bool,
    verified_at: Option<i64>,
    #[allow(dead_code)]
    requires_kyc: bool,
}

// ── Display helpers ───────────────────────────────────────────────────────────

fn shorten(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…{}", &s[..max / 2], &s[s.len() - max / 2..])
    }
}

fn lamports_to_sol(l: u64) -> f64 {
    l as f64 / 1_000_000_000.0
}

fn fmt_timestamp(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}

// ── Actions ───────────────────────────────────────────────────────────────────

fn list_tournaments() {
    println!("\n[LIST] Fetching all tournaments…\n");
    let url = format!("{}/tournaments", server_url());
    match client().get(&url).send() {
        Ok(r) if r.status().is_success() => match r.json::<Vec<TournamentSummary>>() {
            Ok(ts) if ts.is_empty() => println!("  No tournaments found."),
            Ok(ts) => {
                println!("┌────────────┬─────────────────────────┬──────────┬─────────────┬───────────────┐");
                println!("│ ID         │ Name                    │ Status   │ Players     │ Entry Fee     │");
                println!("├────────────┼─────────────────────────┼──────────┼─────────────┼───────────────┤");
                for t in &ts {
                    let fee = if t.entry_fee_lamports == 0 {
                        "FREE".to_string()
                    } else {
                        format!("{:.4} SOL", lamports_to_sol(t.entry_fee_lamports))
                    };
                    println!(
                        "│ {:<10} │ {:<23} │ {:<8} │ {:>3}/{:<3}     │ {:<13} │",
                        t.tournament_id,
                        shorten(&t.name, 23),
                        &t.status[..t.status.len().min(8)],
                        t.registered,
                        t.max_players,
                        fee,
                    );
                }
                println!("└────────────┴─────────────────────────┴──────────┴─────────────┴───────────────┘");
                println!("  {} tournament(s) total.", ts.len());
            }
            Err(e) => println!("  [ERROR] Parse error: {}", e),
        },
        Ok(r) => println!("  [ERROR] HTTP {}", r.status()),
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn create_tournament() {
    println!("\n[CREATE] New Tournament\n");

    // --- Basic info ---
    let tournament_id = read_u64("Tournament ID (unique number)");
    let name = prompt("Tournament name");

    // --- Format ---
    println!("\n  Format options:");
    println!("    1  Single Elimination");
    println!("    2  Swiss");
    let (format_str, swiss_rounds) = loop {
        match prompt("Format [1/2]").as_str() {
            "1" | "" => break ("SingleElimination".to_string(), None),
            "2" => {
                let rounds = loop {
                    let r = read_u16("Number of Swiss rounds (e.g. 5)");
                    if r >= 2 {
                        break r as u8;
                    }
                    println!("  [!] Need at least 2 rounds.");
                };
                break ("Swiss".to_string(), Some(rounds));
            }
            _ => println!("  [!] Enter 1 or 2."),
        }
    };

    // --- Entrant count ---
    println!();
    let max_players = read_max_players();

    // --- Minimum players (optional) ---
    let min_players: Option<u16> = {
        let s = prompt(&format!(
            "Minimum players to start (leave blank for {})",
            max_players
        ));
        s.parse().ok()
    };

    // --- Entry fee ---
    println!("\n  Entry fee examples:");
    println!("    0          = FREE (no prizes)");
    println!("    10000000   = 0.01 SOL");
    println!("    100000000  = 0.1 SOL");
    println!("    500000000  = 0.5 SOL");
    let entry_fee_lamports = read_u64("Entry fee (lamports)");
    let is_free = entry_fee_lamports == 0;

    // --- Prize shares ---
    println!();
    let prize_shares = read_prize_shares(max_players, is_free);

    // --- ELO range ---
    println!("\n  ELO gating (optional — leave blank to allow all ratings)");
    let elo_min = read_u32_opt("Minimum ELO");
    let elo_max = read_u32_opt("Maximum ELO");

    // --- KYC / CACF ---
    println!("\n  CACF KYC — require players to have completed identity verification?");
    println!("  (Players must register via /identity/register on the website or in-game)");
    let kyc_required = confirm("Require KYC/CACF for all entrants?");

    // --- Schedule ---
    println!();
    let scheduled_at = read_scheduled_at();

    // --- Summary ---
    println!("\n  ┌── Tournament Summary ──────────────────────────────────┐");
    println!("  │ ID:        {:<44}│", tournament_id);
    println!("  │ Name:      {:<44}│", shorten(&name, 44));
    println!(
        "  │ Format:    {:<44}│",
        if swiss_rounds.is_some() {
            format!("Swiss ({} rounds)", swiss_rounds.unwrap())
        } else {
            "Single Elimination".to_string()
        }
    );
    println!(
        "  │ Players:   max={}, min={:<36}│",
        max_players,
        min_players
            .map(|n| n.to_string())
            .unwrap_or_else(|| max_players.to_string())
    );
    println!(
        "  │ Entry fee: {:<44}│",
        if is_free {
            "FREE".to_string()
        } else {
            format!("{:.4} SOL", lamports_to_sol(entry_fee_lamports))
        }
    );
    println!(
        "  │ Prizes:    {:.0}% / {:.0}% / {:.0}% / {:.0}%{:<31}│",
        prize_shares[0] as f32 / 100.0,
        prize_shares[1] as f32 / 100.0,
        prize_shares[2] as f32 / 100.0,
        prize_shares[3] as f32 / 100.0,
        ""
    );
    println!(
        "  │ ELO range: {:<44}│",
        match (elo_min, elo_max) {
            (Some(mn), Some(mx)) => format!("{} – {}", mn, mx),
            (Some(mn), None) => format!("{} – (no max)", mn),
            (None, Some(mx)) => format!("(no min) – {}", mx),
            (None, None) => "Unrestricted".to_string(),
        }
    );
    println!(
        "  │ KYC req'd: {:<44}│",
        if kyc_required { "YES (CACF)" } else { "No" }
    );
    println!(
        "  │ Scheduled: {:<44}│",
        scheduled_at
            .map(fmt_timestamp)
            .unwrap_or_else(|| "Immediate (open now)".to_string())
    );
    println!("  └────────────────────────────────────────────────────────┘");

    if !confirm("\nConfirm creation?") {
        println!("  Aborted.");
        return;
    }

    let req = CreateTournamentReq {
        tournament_id,
        name: name.clone(),
        entry_fee_lamports,
        max_players,
        format: format_str,
        swiss_rounds,
        prize_shares: Some(prize_shares),
        elo_min,
        elo_max,
        min_players,
        scheduled_at,
        kyc_required,
    };

    let url = format!("{}/admin/tournament/create", server_url());
    match client().post(&url).json(&req).send() {
        Ok(r) if r.status().is_success() => {
            println!(
                "  [OK] Tournament '{}' (ID {}) created.",
                name, tournament_id
            );
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            println!("  [ERROR] HTTP {} — {}", status, body);
        }
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn view_tournament() {
    let id = read_u64("Tournament ID");
    let url = format!("{}/tournament/{}", server_url(), id);
    match client().get(&url).send() {
        Ok(r) if r.status().is_success() => match r.json::<TournamentDetail>() {
            Ok(t) => print_tournament_detail(&t),
            Err(e) => println!("  [ERROR] Parse error: {}", e),
        },
        Ok(r) if r.status() == 404 => println!("  [ERROR] Tournament {} not found.", id),
        Ok(r) => println!("  [ERROR] HTTP {}", r.status()),
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn print_tournament_detail(t: &TournamentDetail) {
    let is_free = t.entry_fee_lamports == 0;
    let prize_pool = t
        .prize_pool
        .unwrap_or(t.entry_fee_lamports * t.players.len() as u64);

    println!(
        "\n  ┌── Tournament #{} ─────────────────────────────────────┐",
        t.tournament_id
    );
    println!("  │ Name:      {:<44}│", shorten(&t.name, 44));
    println!("  │ Status:    {:<44}│", t.status);
    println!(
        "  │ Players:   {:>3}/{:<41}│",
        t.players.len(),
        t.max_players
    );
    println!(
        "  │ Entry fee: {:<44}│",
        if is_free {
            "FREE".to_string()
        } else {
            format!("{:.4} SOL", lamports_to_sol(t.entry_fee_lamports))
        }
    );
    if !is_free {
        println!(
            "  │ Prize pool:{:<44}│",
            format!("{:.4} SOL", lamports_to_sol(prize_pool))
        );
        println!(
            "  │ Prize dist:{:.0}% / {:.0}% / {:.0}% / {:.0}%{:<31}│",
            t.prize_shares[0] as f32 / 100.0,
            t.prize_shares[1] as f32 / 100.0,
            t.prize_shares[2] as f32 / 100.0,
            t.prize_shares[3] as f32 / 100.0,
            ""
        );
    }
    println!(
        "  │ KYC req'd: {:<44}│",
        if t.kyc_required.unwrap_or(false) {
            "YES (CACF)"
        } else {
            "No"
        }
    );
    if let Some(ts) = t.scheduled_at {
        println!("  │ Scheduled: {:<44}│", fmt_timestamp(ts));
    }
    if let (Some(mn), Some(mx)) = (t.elo_min, t.elo_max) {
        println!("  │ ELO range: {:<44}│", format!("{} – {}", mn, mx));
    }
    if let Some(ref w) = t.winner {
        println!("  │ Winner:    {:<44}│", shorten(w, 44));
    }
    if let Some(ref p) = t.second_place {
        println!("  │ 2nd place: {:<44}│", shorten(p, 44));
    }
    if let Some(ref p) = t.third_place {
        println!("  │ 3rd place: {:<44}│", shorten(p, 44));
    }
    if let Some(ref p) = t.fourth_place {
        println!("  │ 4th place: {:<44}│", shorten(p, 44));
    }
    println!("  ├── Registered Players ──────────────────────────────────┤");
    if t.players.is_empty() {
        println!("  │ (none yet)                                              │");
    } else {
        let elos = t.player_elos.as_deref().unwrap_or(&[]);
        for (i, p) in t.players.iter().enumerate() {
            let elo_str = elos
                .get(i)
                .map(|e| format!("ELO {}", e))
                .unwrap_or_default();
            println!("  │ #{:<3} {:<36} {:<10}│", i + 1, shorten(p, 36), elo_str);
        }
    }
    println!("  └────────────────────────────────────────────────────────┘");
}

fn view_matches() {
    let id = read_u64("Tournament ID");
    let url = format!("{}/tournament/{}/bracket", server_url(), id);
    match client().get(&url).send() {
        Ok(r) if r.status().is_success() => {
            match r.json::<serde_json::Value>() {
                Ok(v) => {
                    let matches = match v.get("matches").and_then(|m| m.as_array()) {
                        Some(ms) => ms.clone(),
                        None => {
                            println!("  No bracket data.");
                            return;
                        }
                    };
                    println!("\n  ┌── Matches ─────────────────────────────────────────────────────────┐");
                    println!(
                        "  │ {:^4} │ {:^5} │ {:^30} │ {:^30} │",
                        "Idx", "Round", "White", "Black"
                    );
                    println!(
                        "  ├──────┼───────┼────────────────────────────────┼────────────────────┤"
                    );
                    for (i, m_val) in matches.iter().enumerate() {
                        if m_val.is_null() {
                            println!("  │ {:>4} │  TBD  │ {:<30} │ {:<30} │", i, "-", "-");
                            continue;
                        }
                        let white = m_val["player_white"].as_str().unwrap_or("TBD");
                        let black = m_val["player_black"].as_str().unwrap_or("TBD");
                        let round = m_val["round"].as_u64().unwrap_or(0);
                        let status = m_val["status"].as_str().unwrap_or("?");
                        let idx = m_val["match_index"].as_u64().unwrap_or(i as u64);
                        let marker = match status {
                            "Completed" => "[DONE]   ",
                            "Active" => "[ACTIVE] ",
                            _ => "[PENDING]",
                        };
                        println!(
                            "  │ {:>4} │ {:>5} │ {:<30} │ {:<30} │  {}",
                            idx,
                            round,
                            shorten(white, 30),
                            shorten(black, 30),
                            marker
                        );
                        if let Some(winner) = m_val["winner"].as_str() {
                            println!("  │      │       │  Winner: {:<53}│", shorten(winner, 53));
                        }
                    }
                    println!(
                        "  └──────┴───────┴────────────────────────────────┴────────────────────┘"
                    );
                }
                Err(e) => println!("  [ERROR] Parse error: {}", e),
            }
        }
        Ok(r) if r.status() == 404 => println!("  [ERROR] Tournament not found."),
        Ok(r) => println!("  [ERROR] HTTP {}", r.status()),
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn record_result() {
    println!("\n[RECORD RESULT]");
    let tournament_id = read_u64("Tournament ID");
    let match_index = read_usize("Match index");
    let winner = prompt("Winner pubkey (base58)");
    let loser = prompt("Loser pubkey (base58)");

    let url = format!(
        "{}/admin/tournament/{}/record-result",
        server_url(),
        tournament_id
    );
    match client()
        .post(&url)
        .json(&RecordResultReq {
            match_index,
            winner: winner.clone(),
            loser,
        })
        .send()
    {
        Ok(r) if r.status().is_success() => {
            println!("  [OK] Result recorded. Winner: {}", shorten(&winner, 50));
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            println!("  [ERROR] HTTP {} — {}", status, body);
        }
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn set_match_game_id() {
    println!("\n[SET GAME ID]");
    let tournament_id = read_u64("Tournament ID");
    let match_index = read_usize("Match index");
    let game_id = read_u64("Solana game ID");

    let url = format!(
        "{}/admin/tournament/{}/set-match-game-id",
        server_url(),
        tournament_id
    );
    match client()
        .post(&url)
        .json(&SetMatchGameIdReq {
            match_index,
            game_id,
        })
        .send()
    {
        Ok(r) if r.status().is_success() => {
            println!("  [OK] Match {} linked to game {}.", match_index, game_id);
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            println!("  [ERROR] HTTP {} — {}", status, body);
        }
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn start_swiss() {
    println!("\n[START SWISS]");
    let id = read_u64("Tournament ID");
    let url = format!("{}/admin/tournament/{}/initialize-swiss", server_url(), id);
    match client().post(&url).json(&serde_json::json!({})).send() {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>() {
            Ok(v) => println!(
                "  [OK] Swiss tournament started. Players: {}, Rounds: {}",
                v["players"], v["rounds"]
            ),
            Err(_) => println!("  [OK] Swiss tournament started."),
        },
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            println!("  [ERROR] HTTP {} — {}", status, body);
        }
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn check_kyc() {
    println!("\n[KYC STATUS CHECK]");
    println!("  Check whether a player has completed CACF identity verification.");
    let pubkey = prompt("Player wallet pubkey (base58)");
    if pubkey.is_empty() {
        println!("  Aborted.");
        return;
    }

    let url = format!("{}/identity/status/{}", server_url(), pubkey);
    match client().get(&url).send() {
        Ok(r) if r.status().is_success() => match r.json::<KycStatus>() {
            Ok(k) => {
                let status = if k.verified {
                    "VERIFIED"
                } else {
                    "NOT VERIFIED"
                };
                let when = k
                    .verified_at
                    .map(fmt_timestamp)
                    .unwrap_or_else(|| "—".to_string());
                println!("  ┌─ KYC / CACF Status ───────────────────────────────┐");
                println!("  │ Wallet:    {:<42}│", shorten(&pubkey, 42));
                println!("  │ Status:    {:<42}│", status);
                println!("  │ Verified:  {:<42}│", when);
                println!("  └───────────────────────────────────────────────────┘");
            }
            Err(e) => println!("  [ERROR] Parse error: {}", e),
        },
        Ok(r) if r.status() == 404 => {
            println!("  [INFO] Wallet not found in KYC vault — NOT verified.");
        }
        Ok(r) => println!("  [ERROR] HTTP {}", r.status()),
        Err(e) => println!("  [ERROR] {}", e),
    }
}

fn batch_kyc_check() {
    println!("\n[BATCH KYC CHECK]");
    println!("  Enter player pubkeys one per line. Empty line when done.");
    let mut pubkeys = Vec::new();
    loop {
        let s = prompt("Pubkey");
        if s.is_empty() {
            break;
        }
        pubkeys.push(s);
    }
    if pubkeys.is_empty() {
        println!("  No pubkeys entered.");
        return;
    }

    println!("\n  Checking {} wallet(s)…\n", pubkeys.len());
    let mut pass = 0usize;
    let mut fail = 0usize;
    for pk in &pubkeys {
        let url = format!("{}/identity/status/{}", server_url(), pk);
        match client().get(&url).send() {
            Ok(r) if r.status().is_success() => match r.json::<KycStatus>() {
                Ok(k) if k.verified => {
                    let when = k.verified_at.map(fmt_timestamp).unwrap_or_default();
                    println!("  [PASS] {} — verified {}", shorten(pk, 46), when);
                    pass += 1;
                }
                Ok(_) => {
                    println!("  [FAIL] {} — NOT verified", shorten(pk, 46));
                    fail += 1;
                }
                Err(_) => {
                    println!("  [ERR]  {} — parse error", shorten(pk, 46));
                    fail += 1;
                }
            },
            Ok(r) if r.status() == 404 => {
                println!("  [FAIL] {} — NOT in KYC vault", shorten(pk, 46));
                fail += 1;
            }
            Ok(r) => println!("  [ERR]  {} — HTTP {}", shorten(pk, 46), r.status()),
            Err(e) => println!("  [ERR]  {} — {}", shorten(pk, 46), e),
        }
    }
    println!(
        "\n  Result: {} passed, {} failed / not verified.",
        pass, fail
    );
}

fn calculate_prizes() {
    let id = read_u64("Tournament ID");
    let url = format!("{}/tournament/{}", server_url(), id);
    match client().get(&url).send() {
        Ok(r) if r.status().is_success() => match r.json::<TournamentDetail>() {
            Ok(t) => {
                let is_free = t.entry_fee_lamports == 0;
                let pool = t
                    .prize_pool
                    .unwrap_or(t.entry_fee_lamports * t.players.len() as u64);
                println!("\n  ┌── Prize Breakdown ─────────────────────────────────┐");
                if is_free {
                    println!("  │ FREE tournament — no monetary prizes.              │");
                } else {
                    println!(
                        "  │ Total pool: {:>10.4} SOL                          │",
                        lamports_to_sol(pool)
                    );
                    let places = [
                        ("1st", t.prize_shares[0], t.winner.as_deref()),
                        ("2nd", t.prize_shares[1], t.second_place.as_deref()),
                        ("3rd", t.prize_shares[2], t.third_place.as_deref()),
                        ("4th", t.prize_shares[3], t.fourth_place.as_deref()),
                    ];
                    println!("  ├─────────────────────────────────────────────────────┤");
                    for (place, bps, wallet) in places {
                        if bps == 0 {
                            continue;
                        }
                        let amount_sol =
                            lamports_to_sol((pool as u128 * bps as u128 / 10000) as u64);
                        let recipient = wallet.unwrap_or("TBD");
                        println!(
                            "  │ {:<4} {:>8.4} SOL  {:<36}│",
                            place,
                            amount_sol,
                            shorten(recipient, 36)
                        );
                    }
                }
                println!("  └────────────────────────────────────────────────────┘");
            }
            Err(e) => println!("  [ERROR] Parse error: {}", e),
        },
        Ok(r) => println!("  [ERROR] HTTP {}", r.status()),
        Err(e) => println!("  [ERROR] {}", e),
    }
}

// ── Menu ──────────────────────────────────────────────────────────────────────

fn print_header() {
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║         XFChess Tournament Admin CLI                              ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!("  Server : {}", server_url());
    println!("  API key: {}…\n", &api_key()[..api_key().len().min(8)]);
}

fn print_menu() {
    println!("┌── Tournaments ────────────────────────────────────────────────────┐");
    println!("│  1.  List all tournaments                                         │");
    println!("│  2.  Create new tournament                                        │");
    println!("│  3.  View tournament details                                      │");
    println!("│  4.  View tournament bracket / matches                            │");
    println!("├── Match Operations ────────────────────────────────────────────────┤");
    println!("│  5.  Record match result                                          │");
    println!("│  6.  Set match game ID                                            │");
    println!("│  7.  Start Swiss tournament (initialize & open round 1)           │");
    println!("├── KYC / CACF ──────────────────────────────────────────────────────┤");
    println!("│  8.  Check single player KYC status                               │");
    println!("│  9.  Batch KYC check (multiple wallets)                           │");
    println!("├── Finance ─────────────────────────────────────────────────────────┤");
    println!("│  10. Calculate prize payout                                       │");
    println!("│                                                                   │");
    println!("│  0.  Exit                                                         │");
    println!("└───────────────────────────────────────────────────────────────────┘");
    print!("\n  Choice: ");
    io::stdout().flush().unwrap();
}

fn main() {
    print_header();
    loop {
        print_menu();
        let choice = {
            let mut buf = String::new();
            io::stdin().read_line(&mut buf).unwrap();
            buf.trim().to_string()
        };
        println!();
        match choice.as_str() {
            "1" => list_tournaments(),
            "2" => create_tournament(),
            "3" => view_tournament(),
            "4" => view_matches(),
            "5" => record_result(),
            "6" => set_match_game_id(),
            "7" => start_swiss(),
            "8" => check_kyc(),
            "9" => batch_kyc_check(),
            "10" => calculate_prizes(),
            "0" | "q" | "quit" | "exit" => {
                println!("  Goodbye.\n");
                break;
            }
            _ => println!("  [!] Unknown option."),
        }
        println!();
    }
}
