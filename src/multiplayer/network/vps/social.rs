//! Social API helpers (friends, presence, lobby invites).

use serde::{Deserialize, Serialize};
use super::client::{client, vps_base};

fn urlenc(s: &str) -> String {
    s.chars().flat_map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
        c => format!("%{:02X}", c as u32).chars().collect(),
    }).collect()
}

// ── Types mirroring the backend ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: String,
    pub from_node_id: String,
    pub from_pubkey: Option<String>,
    pub from_display: String,
    pub to_node_id: Option<String>,
    pub to_pubkey: Option<String>,
    pub message: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub owner_node_id: String,
    pub contact_node_id: String,
    pub contact_pubkey: Option<String>,
    pub contact_display: String,
    pub contact_elo: Option<u16>,
    pub is_online: bool,
    pub last_seen: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub node_id: String,
    pub pubkey: Option<String>,
    pub display_name: String,
    pub status: String, // "online"|"in_game"|"offline"
    pub game_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInvite {
    pub game_id: String,
    pub from_node_id: String,
    pub from_display: String,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPollResponse {
    pub invites: Vec<LobbyInvite>,
    pub next_index: usize,
}

// ── API calls ────────────────────────────────────────────────────────────────

pub fn send_friend_request(
    from_node_id: &str,
    from_pubkey: Option<&str>,
    from_display: &str,
    to_node_id: Option<&str>,
    to_pubkey: Option<&str>,
    message: Option<&str>,
) -> Result<FriendRequest, String> {
    let body = serde_json::json!({
        "from_node_id": from_node_id,
        "from_pubkey": from_pubkey,
        "from_display": from_display,
        "to_node_id": to_node_id,
        "to_pubkey": to_pubkey,
        "message": message,
    });
    let resp = client()?
        .post(format!("{}/friends/requests", vps_base()))
        .json(&body)
        .send()
        .map_err(|e| format!("send_friend_request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("send_friend_request: HTTP {}", resp.status()));
    }
    resp.json::<FriendRequest>().map_err(|e| format!("parse: {e}"))
}

pub fn get_pending_requests(node_id: &str, pubkey: Option<&str>) -> Result<Vec<FriendRequest>, String> {
    let mut qs = format!("?node_id={}", urlenc(node_id));
    if let Some(pk) = pubkey { qs.push_str(&format!("&pubkey={}", urlenc(pk))); }
    let resp = client()?
        .get(format!("{}/friends/requests{}", vps_base(), qs))
        .send()
        .map_err(|e| format!("get_pending_requests: {e}"))?;
    resp.json::<Vec<FriendRequest>>().map_err(|e| format!("parse: {e}"))
}

pub fn respond_friend_request(request_id: &str, accept: bool, responder_node_id: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "action": if accept { "accept" } else { "reject" },
        "responder_node_id": responder_node_id,
    });
    let resp = client()?
        .put(format!("{}/friends/requests/{}", vps_base(), request_id))
        .json(&body)
        .send()
        .map_err(|e| format!("respond_friend_request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("respond_friend_request: HTTP {}", resp.status()));
    }
    Ok(())
}

pub fn get_contacts(node_id: &str, pubkey: Option<&str>) -> Result<Vec<Contact>, String> {
    let mut qs = format!("?node_id={}", urlenc(node_id));
    if let Some(pk) = pubkey { qs.push_str(&format!("&pubkey={}", urlenc(pk))); }
    let resp = client()?
        .get(format!("{}/friends{}", vps_base(), qs))
        .send()
        .map_err(|e| format!("get_contacts: {e}"))?;
    resp.json::<Vec<Contact>>().map_err(|e| format!("parse: {e}"))
}

pub fn remove_contact(owner_node_id: &str, contact_node_id: &str) -> Result<(), String> {
    let resp = client()?
        .delete(format!("{}/friends/{}?node_id={}", vps_base(), contact_node_id, urlenc(owner_node_id)))
        .send()
        .map_err(|e| format!("remove_contact: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("remove_contact: HTTP {}", resp.status()));
    }
    Ok(())
}

pub fn update_presence(p: &Presence) -> Result<(), String> {
    let resp = client()?
        .put(format!("{}/presence", vps_base()))
        .json(p)
        .send()
        .map_err(|e| format!("update_presence: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("update_presence: HTTP {}", resp.status()));
    }
    Ok(())
}

/// GET /presence — fetch everyone currently online (server filters to the last 5 min).
pub fn get_online() -> Result<Vec<Presence>, String> {
    let resp = client()?
        .get(format!("{}/presence", vps_base()))
        .send()
        .map_err(|e| format!("get_online: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("get_online: HTTP {}", resp.status()));
    }
    resp.json::<Vec<Presence>>().map_err(|e| format!("parse: {e}"))
}

pub fn push_lobby_invite(
    game_id: &str,
    from_node_id: &str,
    from_display: &str,
    to_node_id: &str,
) -> Result<(), String> {
    let body = serde_json::json!({
        "game_id": game_id,
        "from_node_id": from_node_id,
        "from_display": from_display,
        "to_node_id": to_node_id,
    });
    let resp = client()?
        .post(format!("{}/friends/invite", vps_base()))
        .json(&body)
        .send()
        .map_err(|e| format!("push_lobby_invite: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("push_lobby_invite: HTTP {}", resp.status()));
    }
    Ok(())
}

pub fn poll_social(node_id: &str, since_index: usize) -> Result<SocialPollResponse, String> {
    let resp = client()?
        .get(format!("{}/social/poll?node_id={}&since_index={}", vps_base(), urlenc(node_id), since_index))
        .send()
        .map_err(|e| format!("poll_social: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("poll_social: HTTP {}", resp.status()));
    }
    resp.json::<SocialPollResponse>().map_err(|e| format!("parse: {e}"))
}

/// GET /region — fetch the backend's region tag + display label.
pub fn fetch_region() -> Result<(String, String), String> {
    let resp = client()?
        .get(format!("{}/region", vps_base()))
        .send()
        .map_err(|e| format!("fetch_region: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("fetch_region: HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().map_err(|e| format!("parse: {e}"))?;
    let region = v["region"].as_str().unwrap_or("unknown").to_string();
    let label  = v["label"].as_str().unwrap_or("Unknown Region").to_string();
    Ok((region, label))
}
