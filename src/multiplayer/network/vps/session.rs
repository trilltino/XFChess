use serde::{Deserialize, Serialize};

use super::client::{client, client_fast, vps_base};

#[derive(Serialize)]
struct CreateSessionReq<'a> {
    game_id: u64,
    wallet_pubkey: &'a str,
}

#[derive(Deserialize)]
struct CreateSessionResp {
    session_pubkey: String,
    #[serde(default)]
    platform_fee_lamports: u64,
}

#[derive(Serialize)]
struct ActivateSessionReq<'a> {
    game_id: u64,
    signed_tx_b64: &'a str,
}

#[derive(Deserialize)]
pub(super) struct SigResp {
    pub sig: String,
    #[serde(default)]
    pub er_endpoint: String,
}

#[derive(Deserialize)]
pub struct SessionStatus {
    pub active: bool,
    pub session_pubkey: String,
}

pub fn create_session(game_id: u64, wallet_pubkey: &str) -> Result<(String, u64), String> {
    let response = client_fast()?
        .post(format!("{}/session/create", vps_base()))
        .json(&CreateSessionReq {
            game_id,
            wallet_pubkey,
        })
        .send()
        .map_err(|e| format!("vps create_session: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps create_session: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<CreateSessionResp>()
        .map_err(|e| format!("vps create_session parse: {e}"))?;
    Ok((resp.session_pubkey, resp.platform_fee_lamports))
}

#[derive(Deserialize)]
struct PlatformFeeResp {
    platform_fee_lamports: u64,
}

pub fn fetch_platform_fee_lamports() -> Result<u64, String> {
    let response = client_fast()?
        .get(format!("{}/api/rates/platform-fee", vps_base()))
        .send()
        .map_err(|e| format!("vps platform_fee: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps platform_fee: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<PlatformFeeResp>()
        .map_err(|e| format!("vps platform_fee parse: {e}"))?;
    Ok(resp.platform_fee_lamports)
}

pub fn activate_session(game_id: u64, signed_tx_bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(signed_tx_bytes);
    let response = client()?
        .post(format!("{}/session/activate", vps_base()))
        .json(&ActivateSessionReq {
            game_id,
            signed_tx_b64: &b64,
        })
        .send()
        .map_err(|e| format!("vps activate_session: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps activate_session: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps activate_session parse: {e}"))?;
    Ok(resp.sig)
}

// NOTE: `sign_and_submit` (POST /session/sign) was removed along with the
// backend endpoint it called. Nothing in this crate ever invoked it —
// delegation goes through `/game/delegate`, which builds the instruction
// server-side — while the endpoint itself would sign *any* transaction handed
// to it with the game's session key. See `backend/src/signing/routes/main.rs`
// (`protected_routes`) for the full reasoning.

pub fn session_status(game_id: u64) -> Result<SessionStatus, String> {
    let resp = client_fast()?
        .get(format!("{}/session/status/{game_id}", vps_base()))
        .send()
        .map_err(|e| format!("vps session_status: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "vps session_status: session not found for game {game_id}"
        ));
    }
    if !resp.status().is_success() {
        return Err(format!(
            "vps session_status: server error {}",
            resp.status()
        ));
    }
    resp.json::<SessionStatus>()
        .map_err(|e| format!("vps session_status parse: {e}"))
}

pub fn abandon_session(game_id: u64) -> Result<(), String> {
    let response = client_fast()?
        .post(format!("{}/session/abandon/{game_id}", vps_base()))
        .send()
        .map_err(|e| format!("vps abandon_session: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps abandon_session: HTTP {status} — {body}"));
    }
    Ok(())
}

// ── Item 8: Global session verify ─────────────────────────────────────────────

pub fn verify_global_session(wallet_pubkey: &str) -> Result<Option<String>, String> {
    let resp = client_fast()?
        .get(format!(
            "{}/api/global-session/{}/verify",
            vps_base(),
            wallet_pubkey
        ))
        .send()
        .map_err(|e| format!("verify_global_session: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("verify_global_session: HTTP {}", resp.status()));
    }
    let data = resp
        .json::<serde_json::Value>()
        .map_err(|e| format!("verify_global_session parse: {e}"))?;
    let active = data
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if active {
        let session_pubkey = data
            .get("session_pubkey")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(session_pubkey)
    } else {
        Ok(None)
    }
}

#[derive(Serialize)]
struct TrackGameReq<'a> {
    game_id: u64,
    wallet_pubkey: &'a str,
}

pub fn track_global_session_game(game_id: u64, wallet_pubkey: &str) -> Result<(), String> {
    let resp = client_fast()?
        .post(format!("{}/api/global-session/track-game", vps_base()))
        .json(&TrackGameReq {
            game_id,
            wallet_pubkey,
        })
        .send()
        .map_err(|e| format!("track_global_session_game: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("track_global_session_game: HTTP {}", resp.status()));
    }
    Ok(())
}
