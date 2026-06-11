//! P2P relay endpoints on the VPS.
//!
//! The VPS acts as a discovery + message-relay for direct P2P games:
//! hosts `announce` open games, players `list`/`join`, either side can
//! `send`/`poll` messages, and `leave` cancels a hosted game.

use serde::{Deserialize, Serialize};

use super::client::{client, vps_base};

#[derive(Serialize)]
struct P2PAnnounceReq<'a> {
    game_id: String,
    host_node_id: &'a str,
    display_name: &'a str,
    stake_amount: f64,
    game_type: &'a str,
    base_time_seconds: u32,
    increment_seconds: u16,
    username: Option<String>,
    elo: Option<u16>,
    region: Option<String>,
    password: Option<String>,
}

#[derive(Serialize)]
struct P2PJoinReq<'a> {
    game_id: String,
    joiner_node_id: &'a str,
    password: Option<String>,
}

/// Optional filter for listing games
#[derive(Default, Serialize)]
pub struct P2PListFilter {
    pub time_min: Option<u32>,
    pub time_max: Option<u32>,
    pub stake_min: Option<f64>,
    pub stake_max: Option<f64>,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    /// "elo_asc"|"elo_desc"|"stake_asc"|"stake_desc"|"time_asc"|"newest"
    pub sort: Option<String>,
}

#[derive(Serialize)]
struct P2PMessageReq<'a> {
    game_id: String,
    from_node_id: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct P2PLeaveReq<'a> {
    game_id: String,
    node_id: &'a str,
}

#[derive(Deserialize)]
pub struct P2PGameListing {
    pub game_id: String,
    pub display_name: String,
    pub stake_amount: f64,
    pub game_type: String,
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub status: String,
    pub username: Option<String>,
    pub elo: Option<u16>,
    pub region: Option<String>,
    #[serde(default = "default_capacity")]
    pub capacity: u8,
    #[serde(default = "default_one")]
    pub players_joined: u8,
    #[serde(default)]
    pub ttl_seconds: i64,
    #[serde(default)]
    pub is_private: bool,
}

fn default_capacity() -> u8 { 2 }
fn default_one() -> u8 { 1 }

#[derive(Deserialize)]
struct P2PJoinResp {
    success: bool,
    host_node_id: Option<String>,
}

#[derive(Deserialize)]
struct P2PPollResp {
    messages: Vec<String>,
    next_index: usize,
}

/// Announce a P2P game to the VPS relay.
pub fn p2p_announce_game(
    game_id: String,
    host_node_id: &str,
    display_name: &str,
    stake_amount: f64,
    game_type: &str,
    base_time_seconds: u32,
    increment_seconds: u16,
    username: Option<String>,
    elo: Option<u16>,
    region: Option<String>,
) -> Result<(), String> {
    p2p_announce_game_private(game_id, host_node_id, display_name, stake_amount,
        game_type, base_time_seconds, increment_seconds, username, elo, region, None)
}

/// Announce a password-protected P2P game.
pub fn p2p_announce_game_with_password(
    game_id: String,
    host_node_id: &str,
    display_name: &str,
    stake_amount: f64,
    game_type: &str,
    base_time_seconds: u32,
    increment_seconds: u16,
    username: Option<String>,
    elo: Option<u16>,
    region: Option<String>,
    password: String,
) -> Result<(), String> {
    p2p_announce_game_private(game_id, host_node_id, display_name, stake_amount,
        game_type, base_time_seconds, increment_seconds, username, elo, region, Some(password))
}

fn p2p_announce_game_private(
    game_id: String,
    host_node_id: &str,
    display_name: &str,
    stake_amount: f64,
    game_type: &str,
    base_time_seconds: u32,
    increment_seconds: u16,
    username: Option<String>,
    elo: Option<u16>,
    region: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    let base = vps_base();
    tracing::info!("[P2P] announcing game to {}", base);
    let resp = client()?
        .post(format!("{}/p2p/announce", base))
        .json(&P2PAnnounceReq {
            game_id,
            host_node_id,
            display_name,
            stake_amount,
            game_type,
            base_time_seconds,
            increment_seconds,
            username,
            elo,
            region,
            password,
        })
        .send()
        .map_err(|e| format!("vps p2p_announce: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_announce: HTTP {status} - {body}"));
    }

    Ok(())
}

/// List available P2P games from VPS relay (no filter).
pub fn p2p_list_games() -> Result<Vec<P2PGameListing>, String> {
    p2p_list_games_filtered(&P2PListFilter::default())
}

/// List available P2P games with optional filter / sort.
pub fn p2p_list_games_filtered(filter: &P2PListFilter) -> Result<Vec<P2PGameListing>, String> {
    let base = vps_base();
    tracing::trace!("[P2P] polling games from {}", base);
    let mut params: Vec<String> = Vec::new();
    if let Some(v) = filter.time_min { params.push(format!("time_min={}", v)); }
    if let Some(v) = filter.time_max { params.push(format!("time_max={}", v)); }
    if let Some(v) = filter.stake_min { params.push(format!("stake_min={}", v)); }
    if let Some(v) = filter.stake_max { params.push(format!("stake_max={}", v)); }
    if let Some(v) = filter.elo_min { params.push(format!("elo_min={}", v)); }
    if let Some(v) = filter.elo_max { params.push(format!("elo_max={}", v)); }
    if let Some(ref s) = filter.sort { params.push(format!("sort={}", s)); }
    let qs = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
    let resp = client()?
        .get(format!("{}/p2p/games{}", base, qs))
        .send()
        .map_err(|e| format!("vps p2p_list_games: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("vps p2p_list_games: HTTP {}", resp.status()));
    }

    resp.json::<Vec<P2PGameListing>>()
        .map_err(|e| format!("vps p2p_list_games parse: {e}"))
}

/// Join a P2P game via VPS relay. Returns the host node ID on success.
pub fn p2p_join_game(game_id: String, joiner_node_id: &str) -> Result<Option<String>, String> {
    p2p_join_game_with_password(game_id, joiner_node_id, None)
}

/// Join a password-protected P2P game.
pub fn p2p_join_game_with_password(game_id: String, joiner_node_id: &str, password: Option<String>) -> Result<Option<String>, String> {
    let resp = client()?
        .post(format!("{}/p2p/join", vps_base()))
        .json(&P2PJoinReq {
            game_id,
            joiner_node_id,
            password,
        })
        .send()
        .map_err(|e| format!("vps p2p_join: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_join: HTTP {status} - {body}"));
    }

    let result = resp
        .json::<P2PJoinResp>()
        .map_err(|e| format!("vps p2p_join parse: {e}"))?;

    if result.success {
        Ok(result.host_node_id)
    } else {
        Err("Join request rejected".to_string())
    }
}

/// Send a P2P message via VPS relay.
pub fn p2p_send_message(game_id: String, from_node_id: &str, message: &str) -> Result<(), String> {
    let resp = client()?
        .post(format!("{}/p2p/message", vps_base()))
        .json(&P2PMessageReq {
            game_id,
            from_node_id,
            message,
        })
        .send()
        .map_err(|e| format!("vps p2p_send_message: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_send_message: HTTP {status} - {body}"));
    }

    #[derive(serde::Deserialize)]
    struct SendResp { success: bool }
    let body: SendResp = resp.json()
        .map_err(|e| format!("vps p2p_send_message parse: {e}"))?;
    if !body.success {
        return Err("vps p2p_send_message: server rejected message (game not found or node_id mismatch)".to_string());
    }

    Ok(())
}

/// Poll for P2P messages from VPS relay. Returns `(messages, next_index)`.
pub fn p2p_poll_messages(
    game_id: String,
    node_id: &str,
    since_index: usize,
) -> Result<(Vec<String>, usize), String> {
    let body = serde_json::json!({
        "game_id": game_id,
        "node_id": node_id,
        "since_index": since_index,
    });

    let resp = client()?
        .post(format!("{}/p2p/poll", vps_base()))
        .json(&body)
        .send()
        .map_err(|e| format!("vps p2p_poll: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_poll: HTTP {status} - {body}"));
    }

    let result = resp
        .json::<P2PPollResp>()
        .map_err(|e| format!("vps p2p_poll parse: {e}"))?;

    Ok((result.messages, result.next_index))
}

/// Send a heartbeat so the lobby doesn't expire on the backend while the host is waiting.
pub fn p2p_heartbeat(game_id: String, host_node_id: &str) -> Result<(), String> {
    let base = vps_base();
    let resp = client()?
        .post(format!("{}/p2p/heartbeat", base))
        .json(&serde_json::json!({ "game_id": game_id, "host_node_id": host_node_id }))
        .send()
        .map_err(|e| format!("vps p2p_heartbeat: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("vps p2p_heartbeat: HTTP {}", resp.status()));
    }
    Ok(())
}

/// Leave or cancel a P2P game on the VPS relay.
pub fn p2p_leave_game(game_id: String, node_id: &str) -> Result<(), String> {
    let resp = client()?
        .post(format!("{}/p2p/leave", vps_base()))
        .json(&P2PLeaveReq { game_id, node_id })
        .send()
        .map_err(|e| format!("vps p2p_leave: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_leave: HTTP {status} - {body}"));
    }

    Ok(())
}
