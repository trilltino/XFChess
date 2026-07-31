//! Session lifecycle endpoints on the VPS.
//!
//! Covers session keypair creation, activation (submitting the wallet-signed
//! setup tx), signing arbitrary tx bytes with the session key, and session
//! status lookup.

use serde::{Deserialize, Serialize};

use super::client::{client, vps_base};

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

#[derive(Serialize)]
struct SignReq<'a> {
    game_id: u64,
    tx_b64: &'a str,
}

#[derive(Deserialize)]
pub(super) struct SigResp {
    pub sig: String,
}

#[derive(Deserialize)]
pub struct SessionStatus {
    pub active: bool,
    pub session_pubkey: String,
}

/// Ask VPS to create (or return existing) session keypair for `game_id`.
/// Returns `(session_pubkey_base58, platform_fee_lamports)`.
/// `platform_fee_lamports` is calculated by the backend from the live SOL/GBP rate (10p per player = 20p total).
pub fn create_session(game_id: u64, wallet_pubkey: &str) -> Result<(String, u64), String> {
    let resp = client()?
        .post(format!("{}/session/create", vps_base()))
        .json(&CreateSessionReq {
            game_id,
            wallet_pubkey,
        })
        .send()
        .map_err(|e| format!("vps create_session: {e}"))?
        .json::<CreateSessionResp>()
        .map_err(|e| format!("vps create_session parse: {e}"))?;
    Ok((resp.session_pubkey, resp.platform_fee_lamports))
}

#[derive(Deserialize)]
struct PlatformFeeResp {
    platform_fee_lamports: u64,
}

/// Fetch the flat per-game platform fee in lamports (live SOL/GBP rate).
/// Used by the global-session create-game path, which — unlike
/// `create_session` above — has no per-game backend session round-trip to
/// piggyback the fee on. Same backend-computed figure either way
/// (`rates::PLATFORM_FEE_GBP`), so the two paths can never silently diverge.
pub fn fetch_platform_fee_lamports() -> Result<u64, String> {
    let resp = client()?
        .get(format!("{}/api/rates/platform-fee", vps_base()))
        .send()
        .map_err(|e| format!("vps platform_fee: {e}"))?
        .json::<PlatformFeeResp>()
        .map_err(|e| format!("vps platform_fee parse: {e}"))?;
    Ok(resp.platform_fee_lamports)
}

/// Submit the wallet-signed setup TX (create_game / join_game + authorize_session_key).
/// VPS submits to chain and funds the session key.
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

/// Ask VPS to sign a pre-built TX with the session key and submit it.
/// Used for delegation: client builds the complex instruction, VPS signs.
pub fn sign_and_submit(game_id: u64, tx_bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(tx_bytes);
    let resp = client()?
        .post(format!("{}/session/sign", vps_base()))
        .json(&SignReq {
            game_id,
            tx_b64: &b64,
        })
        .send()
        .map_err(|e| format!("vps sign_and_submit: {e}"))?
        .json::<SigResp>()
        .map_err(|e| format!("vps sign_and_submit parse: {e}"))?;
    Ok(resp.sig)
}

/// Query session status from VPS.
pub fn session_status(game_id: u64) -> Result<SessionStatus, String> {
    let resp = client()?
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

// ── Item 8: Global session verify ─────────────────────────────────────────────

/// Check whether the VPS holds an active global session for `wallet_pubkey`.
/// Returns `Ok(Some(session_pubkey))` if active, `Ok(None)` if not, `Err` on network failure.
pub fn verify_global_session(wallet_pubkey: &str) -> Result<Option<String>, String> {
    let resp = client()?
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

/// Tell the backend a game was just created/joined via the global-session
/// flow, so `settlement_worker` can discover and auto-settle it — see
/// `routes::global_session::track_game`'s doc comment for why this exists.
/// Best-effort: `finalize_game`/`undelegate_game` don't depend on this call
/// having succeeded (they resolve a signer from on-chain state directly), so
/// a failure here only means this specific game misses out on *automatic*
/// settlement, not that it becomes unsettleable.
pub fn track_global_session_game(game_id: u64, wallet_pubkey: &str) -> Result<(), String> {
    let resp = client()?
        .post(format!("{}/api/global-session/track-game", vps_base()))
        .json(&TrackGameReq {
            game_id,
            wallet_pubkey,
        })
        .send()
        .map_err(|e| format!("track_global_session_game: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "track_global_session_game: HTTP {}",
            resp.status()
        ));
    }
    Ok(())
}
