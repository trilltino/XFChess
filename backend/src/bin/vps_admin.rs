//! VPS Admin Console - Tournament Management CLI
//!
//! Run with: cargo run --bin vps_admin
//!
//! Requires ADMIN_API_KEY environment variable to be set.
//!
//! Provides admin commands for:
//! - Creating tournaments (8/16/32/64/128 players)
//! - Listing active/completed tournaments
//! - Viewing tournament brackets
//! - Recording match results
//! - Setting match game IDs
//! - Cancelling tournaments
//! - Checking prize distributions

#![allow(dead_code)]

use std::io::{self, Write};
use std::env;
use serde::{Deserialize, Serialize};

const VPS_DEFAULT_URL: &str = "https://unrejuvenated-philologically-trudi.ngrok-free.app";

fn vps_base() -> String {
    match env::var("SIGNING_SERVICE_URL") {
        Ok(url) => url,
        Err(e) => {
            eprintln!("SIGNING_SERVICE_URL error: {:?}, using default: {}", e, VPS_DEFAULT_URL);
            VPS_DEFAULT_URL.to_string()
        }
    }
}

fn get_api_key() -> String {
    env::var("ADMIN_API_KEY").expect("ADMIN_API_KEY environment variable must be set")
}

fn client() -> reqwest::blocking::Client {
    let api_key = get_api_key();
    
    let mut h = reqwest::header::HeaderMap::new();
    h.insert("ngrok-skip-browser-warning", reqwest::header::HeaderValue::from_static("true"));
    h.insert("Content-Type", reqwest::header::HeaderValue::from_static("application/json"));
    
    match reqwest::header::HeaderValue::from_str(&api_key) {
        Ok(val) => h.insert("X-API-Key", val),
        Err(e) => {
            eprintln!("Invalid API key format: {:?}. API key must be valid HTTP header value.", e);
            panic!("Cannot create client with invalid API key");
        }
    };
    
    reqwest::blocking::Client::builder()
        .default_headers(h)
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("Failed to build HTTP client - check system configuration")
}

// ── API Types ────────────────────────────────────────────────────────────────

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
    player_elos: Vec<u32>,
    prize_pool: u64,
    prize_shares: [u16; 4],
    winner: Option<String>,
    second_place: Option<String>,
    third_place: Option<String>,
    fourth_place: Option<String>,
    matches: Vec<Option<MatchDetail>>,
    current_round: u8,
}

#[derive(Debug, Deserialize)]
struct MatchDetail {
    match_index: u16,
    round: u8,
    player_white: Option<String>,
    player_black: Option<String>,
    winner: Option<String>,
    game_id: Option<u64>,
    status: String,
    next_match_for_winner: Option<u16>,
    next_match_slot: u8,
}

#[derive(Debug, Serialize)]
struct CreateTournamentReq {
    tournament_id: u64,
    name: String,
    entry_fee_lamports: u64,
    max_players: u16,
    prize_shares: Option<[u16; 4]>,
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

// ── Console UI ─────────────────────────────────────────────────────────────────

fn print_header() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║           XFChess VPS Tournament Admin Console                   ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!(" VPS: {}\n", vps_base());
}

fn print_menu() {
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│  TOURNAMENT MANAGEMENT                                          │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│  1. List all tournaments                                        │");
    println!("│  2. Create new tournament                                       │");
    println!("│  3. View tournament details & bracket                           │");
    println!("│  4. View tournament matches                                     │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│  MATCH OPERATIONS                                               │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│  5. Record match result                                         │");
    println!("│  6. Set match game ID                                           │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│  ADMIN                                                          │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│  7. Cancel tournament (refund all)                              │");
    println!("│  8. Calculate prize payout                                      │");
    println!("│                                                                 │");
    println!("│  0. Exit                                                        │");
    println!("└─────────────────────────────────────────────────────────────────┘");
    print!("\nEnter choice: ");
    io::stdout().flush().unwrap();
}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn read_u64(prompt: &str) -> u64 {
    loop {
        print!("{}: ", prompt);
        io::stdout().flush().unwrap();
        match read_line().parse::<u64>() {
            Ok(n) => return n,
            Err(_) => println!("  Invalid number, try again."),
        }
    }
}

fn read_u16(prompt: &str) -> u16 {
    loop {
        print!("{}: ", prompt);
        io::stdout().flush().unwrap();
        match read_line().parse::<u16>() {
            Ok(n) => return n,
            Err(_) => println!("  Invalid number, try again."),
        }
    }
}

fn read_usize(prompt: &str) -> usize {
    loop {
        print!("{}: ", prompt);
        io::stdout().flush().unwrap();
        match read_line().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => println!("  Invalid number, try again."),
        }
    }
}

fn read_string(prompt: &str) -> String {
    print!("{}: ", prompt);
    io::stdout().flush().unwrap();
    read_line()
}

fn confirm(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();
    let input = read_line().to_lowercase();
    input == "y" || input == "yes"
}

// ── API Functions ─────────────────────────────────────────────────────────────

fn list_tournaments() {
    println!("\n[LIST] Fetching tournaments...\n");
    
    match client()
        .get(format!("{}/tournaments", vps_base()))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Vec<TournamentSummary>>() {
                Ok(tournaments) => {
                    if tournaments.is_empty() {
                        println!("  No tournaments found.");
                        return;
                    }
                    
                    println!("┌────────────┬─────────────────────────┬──────────┬─────────────┬───────────────┬────────────────┐");
                    println!("│ ID         │ Name                    │ Status   │ Players     │ Entry Fee     │ Prize Pool     │");
                    println!("├────────────┼─────────────────────────┼──────────┼─────────────┼───────────────┼────────────────┤");
                    
                    for t in tournaments {
                        let fee_display = if t.entry_fee_lamports == 0 {
                            "FREE".to_string()
                        } else {
                            format!("{:>6.3} SOL", t.entry_fee_lamports as f64 / 1_000_000_000.0)
                        };
                        let prize_display = if t.entry_fee_lamports == 0 {
                            "N/A".to_string()
                        } else {
                            format!("{:>8.3} SOL", (t.registered as u64 * t.entry_fee_lamports) as f64 / 1_000_000_000.0)
                        };
                        println!("│ {:<10} │ {:<23} │ {:<8} │ {:>3}/{:<3}     │ {:<13} │ {:<14} │",
                            t.tournament_id,
                            &t.name[..t.name.len().min(23)],
                            t.status,
                            t.registered,
                            t.max_players,
                            fee_display,
                            prize_display
                        );
                    }
                    println!("└────────────┴─────────────────────────┴──────────┴─────────────┴───────────────┴────────────────┘");
                }
                Err(e) => println!("  [ERROR] Error parsing response: {}", e),
            }
        }
        Ok(resp) => println!("  [ERROR] HTTP error: {}", resp.status()),
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn create_tournament() {
    println!("\n[CREATE] Create New Tournament\n");
    
    let tournament_id = read_u64("Tournament ID (unique number)");
    let name = read_string("Tournament name");
    
    println!("\nEntry fee options:");
    println!("  0          = FREE tournament (practice, no prizes)");
    println!("  10000000   = 0.01 SOL (example: ~2 GBP at 200 GBP/SOL)");
    println!("  100000000  = 0.1 SOL");
    println!("  500000000  = 0.5 SOL");
    let entry_fee_lamports = read_u64("Entry fee (lamports, 1 SOL = 1_000_000_000)");
    let is_free = entry_fee_lamports == 0;
    
    println!("\nPlayer count options: 8, 16, 32, 64, 128");
    let max_players = loop {
        let n = read_u16("Max players");
        if [8, 16, 32, 64, 128].contains(&n) {
            break n;
        }
        println!("  Invalid choice. Must be 8, 16, 32, 64, or 128.");
    };
    
    // Default prize shares
    let prize_shares: [u16; 4] = if is_free {
        println!("\n[FREE] No prize distribution for free tournaments.");
        [0, 0, 0, 0]
    } else if max_players >= 16 {
        println!("\nDefault prize distribution for {} players: 50%/30%/15%/5%", max_players);
        if confirm("Use custom prize shares?") {
            let first = read_u16("1st place (basis points, 10000 = 100%)") as u16;
            let second = read_u16("2nd place (basis points)") as u16;
            let third = read_u16("3rd place (basis points)") as u16;
            let fourth = read_u16("4th place (basis points)") as u16;
            [first, second, third, fourth]
        } else {
            [5000, 3000, 1500, 500]
        }
    } else {
        println!("\n8-player tournament: Winner-take-all (100%)");
        [10000, 0, 0, 0]
    };
    
    let req = CreateTournamentReq {
        tournament_id,
        name: name.clone(),
        entry_fee_lamports,
        max_players,
        prize_shares: Some(prize_shares),
    };
    
    println!("\n[SENDING] Creating tournament...");
    match client()
        .post(format!("{}/admin/tournament/create", vps_base()))
        .json(&req)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            println!("  [OK] Tournament '{}' created successfully!", name);
            println!("     ID: {}", tournament_id);
            println!("     Players: {}", max_players);
            if is_free {
                println!("     Entry Fee: FREE (Practice Tournament)");
            } else {
                println!("     Entry Fee: {:.6} SOL", entry_fee_lamports as f64 / 1_000_000_000.0);
                println!("     Prize Pool: {:.3} SOL ({} x {:.6} SOL)",
                    (max_players as f64 * entry_fee_lamports as f64) / 1_000_000_000.0,
                    max_players,
                    entry_fee_lamports as f64 / 1_000_000_000.0
                );
            }
            println!("     Prize Shares: {:?}", prize_shares);
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            println!("  [ERROR] Failed: HTTP {} - {}", status, body);
        }
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn view_tournament() {
    let id = read_u64("Tournament ID");
    
    println!("\n[VIEW] Fetching tournament {}...\n", id);
    
    match client()
        .get(format!("{}/tournament/{}", vps_base(), id))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<TournamentDetail>() {
                Ok(t) => {
                    // Calculate actual prize pool from entry fees (on-chain model)
                    let actual_prize_pool = t.entry_fee_lamports * t.players.len() as u64;
                    let is_free = t.entry_fee_lamports == 0;
                    
                    println!("┌─────────────────────────────────────────────────────────────────┐");
                    println!("│ TOURNAMENT #{} {:<48}│", t.tournament_id, &t.name[..t.name.len().min(48)]);
                    println!("├─────────────────────────────────────────────────────────────────┤");
                    println!("│ Status:     {:<50}│", t.status);
                    println!("│ Players:    {:>3}/{:<47}│", t.players.len(), t.max_players);
                    
                    if is_free {
                        println!("│ Entry Fee:  FREE (Practice Tournament){:>25}│", "");
                        println!("│ Prize Pool: N/A (No prizes){:>37}│", "");
                    } else {
                        let fee_sol = t.entry_fee_lamports as f64 / 1_000_000_000.0;
                        let pool_sol = actual_prize_pool as f64 / 1_000_000_000.0;
                        println!("│ Entry Fee:  {:>10.6} SOL per player{:>25}│", fee_sol, "");
                        println!("│ Prize Pool: {:>10.3} SOL ({} players x {:.6} SOL){:>10}│", 
                            pool_sol, t.players.len(), fee_sol, "");
                    }
                    
                    println!("│ Prize Dist: {:>4}/{:>4}/{:>4}/{:>4} % (1st/2nd/3rd/4th){:>13}│",
                        t.prize_shares[0] / 100,
                        t.prize_shares[1] / 100,
                        t.prize_shares[2] / 100,
                        t.prize_shares[3] / 100,
                        ""
                    );
                    
                    if let Some(ref w) = t.winner {
                        println!("│ Winner:     {:<50}│", &w[..w.len().min(50)]);
                    }
                    if let Some(ref s) = t.second_place {
                        println!("│ 2nd Place:  {:<50}│", &s[..s.len().min(50)]);
                    }
                    if let Some(ref t3) = t.third_place {
                        println!("│ 3rd Place:  {:<50}│", &t3[..t3.len().min(50)]);
                    }
                    if let Some(ref t4) = t.fourth_place {
                        println!("│ 4th Place:  {:<50}│", &t4[..t4.len().min(50)]);
                    }
                    
                    println!("├─────────────────────────────────────────────────────────────────┤");
                    println!("│ REGISTERED PLAYERS (sorted by ELO):                             │");
                    println!("├─────────────────────────────────────────────────────────────────┤");
                    
                    for (i, (player, elo)) in t.players.iter().zip(t.player_elos.iter()).enumerate() {
                        let seed = i + 1;
                        let short_id = if player.len() > 40 {
                            format!("{}...{}", &player[..20], &player[player.len()-17..])
                        } else {
                            player.clone()
                        };
                        println!("│ #{} {:<6} ELO: {:<5} {:<43}│", seed, "", elo, short_id);
                    }
                    
                    println!("└─────────────────────────────────────────────────────────────────┘");
                }
                Err(e) => println!("  [ERROR] Error parsing: {}", e),
            }
        }
        Ok(resp) if resp.status() == 404 => println!("  [ERROR] Tournament not found."),
        Ok(resp) => println!("  [ERROR] HTTP error: {}", resp.status()),
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn view_matches() {
    let id = read_u64("Tournament ID");
    
    println!("\n[MATCHES] Fetching matches for tournament {}...\n", id);
    
    match client()
        .get(format!("{}/tournament/{}/bracket", vps_base(), id))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<TournamentDetail>() {
                Ok(t) => {
                    if t.matches.is_empty() {
                        println!("  No matches yet. Tournament may not have started.");
                        return;
                    }
                    
                    println!("┌─────────────────────────────────────────────────────────────────────────────────┐");
                    println!("│ MATCHES                                                                          │");
                    println!("├──────┬───────┬────────────────────────────────┬────────────────────────────────┤");
                    println!("│ Idx  │ Round │ White                          │ Black                          │");
                    println!("├──────┼───────┼────────────────────────────────┼────────────────────────────────┤");
                    
                    for (i, m_opt) in t.matches.iter().enumerate() {
                        if let Some(ref m) = m_opt {
                            let white = m.player_white.as_ref()
                                .map(|p| format!("{:.20}...", p))
                                .unwrap_or_else(|| "TBD".to_string());
                            let black = m.player_black.as_ref()
                                .map(|p| format!("{:.20}...", p))
                                .unwrap_or_else(|| "TBD".to_string());
                            
                            let status_marker = if m.winner.is_some() {
                                "[DONE]"
                            } else if m.game_id.is_some() {
                                "[ACTIVE]"
                            } else {
                                "[PENDING]"
                            };
                            
                            println!("│ {:>3}{} │ {:>5} │ {:<30} │ {:<30} │",
                                m.match_index, status_marker, m.round, white, black);
                            
                            if let Some(ref winner) = m.winner {
                                println!("│      │       │ Winner: {:<54} │", &winner[..winner.len().min(54)]);
                            }
                            if let Some(game_id) = m.game_id {
                                println!("│      │       │ Game ID: {:<53} │", game_id);
                            }
                        } else {
                            println!("│ {:>3}  │ TBD   │ {:<30} │ {:<30} │", i, "-", "-");
                        }
                    }
                    println!("└──────┴───────┴────────────────────────────────┴────────────────────────────────┘");
                    println!("\nLegend: [PENDING] = Pending, [ACTIVE] = Active, [DONE] = Complete");
                }
                Err(e) => println!("  [ERROR] Error parsing: {}", e),
            }
        }
        Ok(resp) if resp.status() == 404 => println!("  [ERROR] Tournament not found."),
        Ok(resp) => println!("  [ERROR] HTTP error: {}", resp.status()),
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn record_result() {
    let tournament_id = read_u64("Tournament ID");
    let match_index = read_usize("Match index");
    let winner = read_string("Winner pubkey (base58)");
    let loser = read_string("Loser pubkey (base58)");
    
    let req = RecordResultReq {
        match_index,
        winner: winner.clone(),
        loser: loser.clone(),
    };
    
    println!("\n[RECORD] Recording result for match {}...", match_index);
    
    match client()
        .post(format!("{}/admin/tournament/{}/record-result", vps_base(), tournament_id))
        .json(&req)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            println!("  [OK] Result recorded!");
            println!("     Winner: {}", &winner[..winner.len().min(50)]);
            println!("     Loser:  {}", &loser[..loser.len().min(50)]);
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            println!("  [ERROR] Failed: HTTP {} - {}", status, body);
        }
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn set_match_game_id() {
    let tournament_id = read_u64("Tournament ID");
    let match_index = read_usize("Match index");
    let game_id = read_u64("Game ID (Solana on-chain game ID)");

    let req = SetMatchGameIdReq {
        match_index,
        game_id,
    };

    println!("\n[LINK] Setting game ID {} for match {}...", game_id, match_index);

    match client()
        .post(format!("{}/admin/tournament/{}/set-match-game-id", vps_base(), tournament_id))
        .json(&req)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            println!("  [OK] Game ID set! Match {} is now linked to game {}.", match_index, game_id);
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            println!("  [ERROR] Failed: HTTP {} - {}", status, body);
        }
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn calculate_prizes() {
    let id = read_u64("Tournament ID");

    println!("\n[PRIZES] Fetching prize info for tournament {}...\n", id);

    match client()
        .get(format!("{}/tournament/{}", vps_base(), id))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<TournamentDetail>() {
                Ok(t) => {
                    let is_free = t.entry_fee_lamports == 0;
                    let actual_prize_pool = t.entry_fee_lamports * t.players.len() as u64;
                    let pool_sol = actual_prize_pool as f64 / 1_000_000_000.0;

                    println!("┌─────────────────────────────────────────────────────────────────┐");
                    println!("│ PRIZE BREAKDOWN                                                │");
                    println!("├─────────────────────────────────────────────────────────────────┤");

                    if is_free {
                        println!("│ This is a FREE practice tournament.                             │");
                        println!("│ No monetary prizes - play for fun and bragging rights!        │");
                        println!("├─────────────────────────────────────────────────────────────────┤");
                        println!("│ Places:                                                         │");
                        println!("│   1st: {}                                                       │",
                            t.winner.as_ref().map(|w| &w[..w.len().min(58)])
                                .unwrap_or("TBD"));
                        println!("│   2nd: {}                                                       │",
                            t.second_place.as_ref().map(|w| &w[..w.len().min(58)])
                                .unwrap_or("TBD"));
                        if t.third_place.is_some() {
                            println!("│   3rd: {}                                                       │",
                                t.third_place.as_ref().map(|w| &w[..w.len().min(58)])
                                    .unwrap_or("TBD"));
                        }
                        if t.fourth_place.is_some() {
                            println!("│   4th: {}                                                       │",
                                t.fourth_place.as_ref().map(|w| &w[..w.len().min(58)])
                                    .unwrap_or("TBD"));
                        }
                        println!("└─────────────────────────────────────────────────────────────────┘");
                        return;
                    }

                    println!("│ Total Prize Pool: {:>10.3} SOL                                 │", pool_sol);
                    println!("│ Distribution:     {:>4}.{:>02}% / {:>4}.{:>02}% / {:>4}.{:>02}% / {:>4}.{:>02}%          │",
                        t.prize_shares[0] / 100, t.prize_shares[0] % 100,
                        t.prize_shares[1] / 100, t.prize_shares[1] % 100,
                        t.prize_shares[2] / 100, t.prize_shares[2] % 100,
                        t.prize_shares[3] / 100, t.prize_shares[3] % 100,
                    );
                    println!("├─────────────────────────────────────────────────────────────────┤");

                    let prizes = [
                        ("1st Place", t.prize_shares[0], t.winner.as_ref()),
                        ("2nd Place", t.prize_shares[1], t.second_place.as_ref()),
                        ("3rd Place", t.prize_shares[2], t.third_place.as_ref()),
                        ("4th Place", t.prize_shares[3], t.fourth_place.as_ref()),
                    ];

                    for (place, share, winner) in prizes {
                        if share > 0 {
                            let amount = (actual_prize_pool as u128 * share as u128 / 10000) as f64 / 1_000_000_000.0;
                            let recipient = winner.map(|w| {
                                if w.len() > 44 {
                                    format!("{}...{}", &w[..20], &w[w.len()-21..])
                                } else {
                                    w.to_string()
                                }
                            }).unwrap_or_else(|| "TBD".to_string());

                            println!("│ {:<10} │ {:>8.3} SOL │ {:<46}│", place, amount, recipient);
                        }
                    }

                    println!("└─────────────────────────────────────────────────────────────────┘");
                }
                Err(e) => println!("  [ERROR] Error parsing: {}", e),
            }
        }
        Ok(resp) if resp.status() == 404 => println!("  [ERROR] Tournament not found."),
        Ok(resp) => println!("  [ERROR] HTTP error: {}", resp.status()),
        Err(e) => println!("  [ERROR] Request failed: {}", e),
    }
}

fn cancel_tournament() {
    let id = read_u64("Tournament ID to cancel");
    
    println!("\n[WARNING] This will cancel the tournament and refund all entry fees!");
    
    if !confirm("Are you sure you want to cancel?") {
        println!("  Cancelled.");
        return;
    }
    
    println!("\n[CANCEL] Cancelling tournament {}...", id);
    
    // Note: Cancel requires on-chain transaction with authority signature
    // This is a placeholder - actual implementation needs wallet integration
    println!("  [WARNING] Cancel requires on-chain authority signature.");
    println!("     Use the Solana CLI or game client with authority wallet:");
    println!("     `solana program invoke --program-id <PROGRAM_ID> cancel_tournament <TOURNAMENT_ID>`");
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    print_header();
    
    loop {
        print_menu();
        
        match read_line().as_str() {
            "1" => list_tournaments(),
            "2" => create_tournament(),
            "3" => view_tournament(),
            "4" => view_matches(),
            "5" => record_result(),
            "6" => set_match_game_id(),
            "7" => cancel_tournament(),
            "8" => calculate_prizes(),
            "0" | "q" | "quit" | "exit" => {
                println!("\nGoodbye!\n");
                break;
            }
            _ => println!("\n  Invalid choice. Please try again."),
        }
        
        println!();
    }
}
