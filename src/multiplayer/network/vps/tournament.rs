//! Tournament discovery and registration endpoints on the VPS.
//!
//! Lists all advertised tournaments and joins them (optionally gated by a
//! private-tournament password). Returns the slot the player was placed in.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::client::{client, vps_base};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub registered: usize,
    pub status: String,
    pub is_private: bool,
    /// true for Swiss/knockout, false for posted PvP games.
    pub is_tournament: bool,
    #[serde(default)]
    pub usdc_mint: Option<String>,
    #[serde(default)]
    pub max_players: usize,
    #[serde(default)]
    pub min_elo: u32,
    #[serde(default)]
    pub max_elo: u32,
    #[serde(default)]
    pub round_deadline_at: Option<i64>,
}

/// One on-chain game created by the backend tournament orchestrator — a bracket
/// match whose `game_id` has been set (i.e. the Solana game account exists).
#[derive(Debug, Clone)]
pub struct TournamentGameListing {
    pub tournament_id: u64,
    pub tournament_name: String,
    pub round: u8,
    pub match_index: u16,
    pub white: Option<String>,
    pub black: Option<String>,
    /// Usernames resolved from the players' on-backend profiles, when they have one.
    pub white_name: Option<String>,
    pub black_name: Option<String>,
    pub game_id: u64,
    /// Match status as reported by the backend: "Pending" / "Active" / "Completed".
    pub status: String,
}

impl TournamentGameListing {
    /// Display label for white: username, else truncated pubkey, else "TBD".
    pub fn white_label(&self) -> String {
        player_label(&self.white_name, &self.white)
    }

    /// Display label for black: username, else truncated pubkey, else "TBD".
    pub fn black_label(&self) -> String {
        player_label(&self.black_name, &self.black)
    }
}

fn player_label(name: &Option<String>, pubkey: &Option<String>) -> String {
    if let Some(n) = name {
        return n.clone();
    }
    match pubkey.as_deref() {
        Some(p) if p.len() > 8 => format!("{}…{}", &p[..4], &p[p.len() - 4..]),
        Some(p) => p.to_string(),
        None => "TBD".to_string(),
    }
}

/// Per-process cache of wallet → username lookups. Failed/absent profiles are
/// cached as `None` so a profileless player doesn't get re-queried every poll
/// (a name created mid-session shows up after restart, which is acceptable).
static USERNAME_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn resolve_username(pubkey: &str) -> Option<String> {
    let cache = USERNAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(pubkey).cloned()) {
        return hit;
    }
    let resolved = super::identity::fetch_player_profile(pubkey)
        .ok()
        .map(|p| p.username)
        .filter(|u| !u.is_empty());
    if let Ok(mut c) = cache.lock() {
        c.insert(pubkey.to_string(), resolved.clone());
    }
    resolved
}

/// Subset of the backend bracket match JSON we care about.
#[derive(Deserialize)]
struct BracketMatch {
    match_index: u16,
    round: u8,
    player_white: Option<String>,
    player_black: Option<String>,
    game_id: Option<u64>,
    status: String,
}

/// Fetch every Solana game created by backend tournaments: walks the advertised
/// tournaments, pulls each bracket, and keeps only matches with an on-chain
/// `game_id`. Skips posted-PvP entries (`is_tournament == false`) and
/// tournaments whose bracket can't be fetched.
pub fn list_tournament_games() -> Result<Vec<TournamentGameListing>, String> {
    let tournaments = list_tournaments()?;
    let mut out = Vec::new();
    for t in tournaments.into_iter().filter(|t| t.is_tournament) {
        let resp = match client()?
            .get(format!("{}/tournament/{}/bracket", vps_base(), t.tournament_id))
            .send()
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue, // e.g. not started yet — no bracket, no games
        };
        let Ok(bracket) = resp.json::<serde_json::Value>() else {
            continue;
        };
        let matches: Vec<Option<BracketMatch>> = bracket
            .get("matches")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        for m in matches.into_iter().flatten() {
            let Some(game_id) = m.game_id else { continue };
            let white_name = m.player_white.as_deref().and_then(resolve_username);
            let black_name = m.player_black.as_deref().and_then(resolve_username);
            out.push(TournamentGameListing {
                tournament_id: t.tournament_id,
                tournament_name: t.name.clone(),
                round: m.round,
                match_index: m.match_index,
                white: m.player_white,
                black: m.player_black,
                white_name,
                black_name,
                game_id,
                status: m.status,
            });
        }
    }
    Ok(out)
}

/// Fetch the list of advertised tournaments from the VPS.
pub fn list_tournaments() -> Result<Vec<TournamentSummary>, String> {
    let resp = client()?
        .get(format!("{}/tournaments", vps_base()))
        .send()
        .map_err(|e| format!("vps list_tournaments: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps list_tournaments: HTTP {}", resp.status()));
    }
    resp.json::<Vec<TournamentSummary>>()
        .map_err(|e| format!("vps list_tournaments parse: {e}"))
}

/// Create a VPS session for the white player of a tournament match.
/// Returns the session pubkey that white must include in the `create_game` instruction.
pub fn tournament_session_create_game(
    tournament_id: u64,
    game_id: u64,
    wallet_pubkey: &str,
) -> Result<String, String> {
    let resp = client()?
        .post(format!(
            "{}/tournament/{}/session-create-game",
            vps_base(),
            tournament_id
        ))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_create_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "tournament_session_create_game: HTTP {status} — {body}"
        ));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("tournament_session_create_game parse: {e}"))?;
    data.get("session_pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing session_pubkey in response".to_string())
}

/// Retrieve/create the VPS session for the black player of a tournament match.
/// Returns the same session pubkey that white already created.
pub fn tournament_session_join_game(
    tournament_id: u64,
    game_id: u64,
    wallet_pubkey: &str,
) -> Result<String, String> {
    let resp = client()?
        .post(format!(
            "{}/tournament/{}/session-join-game",
            vps_base(),
            tournament_id
        ))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_join_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "tournament_session_join_game: HTTP {status} — {body}"
        ));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("tournament_session_join_game parse: {e}"))?;
    data.get("session_pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing session_pubkey in response".to_string())
}

/// Join a tournament. Returns the slot (registration position) on success.
pub fn join_tournament(
    tournament_id: u64,
    player_pubkey: &str,
    password: Option<&str>,
) -> Result<u32, String> {
    let mut body = serde_json::json!({
        "player": player_pubkey,
        "elo": 1200
    });
    if let Some(pw) = password {
        body["password"] = serde_json::Value::String(pw.to_string());
    }
    let resp = client()?
        .post(format!("{}/tournament/{}/join", vps_base(), tournament_id))
        .json(&body)
        .send()
        .map_err(|e| format!("vps join_tournament: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps join_tournament: HTTP {status} - {body}"));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("vps join_tournament parse: {e}"))?;
    data.get("slot")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| "Missing slot in join response".to_string())
}
