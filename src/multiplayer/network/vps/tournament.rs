//! Tournament discovery and registration endpoints on the VPS.
//!
//! Lists all advertised tournaments and joins them (optionally gated by a
//! private-tournament password). Returns the slot the player was placed in.

use serde::{Deserialize, Serialize};

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
        .post(format!("{}/tournament/{}/session-create-game", vps_base(), tournament_id))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_create_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("tournament_session_create_game: HTTP {status} — {body}"));
    }
    let data = resp.json::<serde_json::Value>()
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
        .post(format!("{}/tournament/{}/session-join-game", vps_base(), tournament_id))
        .json(&serde_json::json!({ "game_id": game_id, "wallet_pubkey": wallet_pubkey }))
        .send()
        .map_err(|e| format!("tournament_session_join_game: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("tournament_session_join_game: HTTP {status} — {body}"));
    }
    let data = resp.json::<serde_json::Value>()
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
