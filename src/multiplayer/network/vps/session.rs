//! Session lifecycle endpoints on the VPS.
//!
//! Covers session keypair creation, activation (submitting the wallet-signed
//! setup tx), signing arbitrary tx bytes with the session key, session
//! status lookup, and TEE authentication for privacy-enhanced matches.

use serde::{Deserialize, Serialize};

use super::client::{client, vps_base};

const TEE_AUTH_MESSAGE: &str = "Authenticate with MagicBlock TEE";
const TEE_DEVNET_ADDR: &str = "FnE6VJT5QNZdedZPnCoLsARgBwoE6DeJNjBs2H1gySXA";

#[derive(Serialize)]
struct CreateSessionReq<'a> {
    game_id: u64,
    wallet_pubkey: &'a str,
}

#[derive(Deserialize)]
struct CreateSessionResp {
    session_pubkey: String,
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

#[derive(Serialize)]
struct TeeAuthReq<'a> {
    game_id: u64,
    wallet_pubkey: &'a str,
    signature_b64: &'a str,
}

/// Ask VPS to create (or return existing) session keypair for `game_id`.
/// Returns the session pubkey (base58).
pub fn create_session(game_id: u64, wallet_pubkey: &str) -> Result<String, String> {
    let resp = client()?
        .post(format!("{}/session/create", vps_base()))
        .json(&CreateSessionReq { game_id, wallet_pubkey })
        .send()
        .map_err(|e| format!("vps create_session: {e}"))?
        .json::<CreateSessionResp>()
        .map_err(|e| format!("vps create_session parse: {e}"))?;
    Ok(resp.session_pubkey)
}

/// Submit the wallet-signed setup TX (create_game / join_game + authorize_session_key).
/// VPS submits to chain and funds the session key.
pub fn activate_session(game_id: u64, signed_tx_bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(signed_tx_bytes);
    let response = client()?
        .post(format!("{}/session/activate", vps_base()))
        .json(&ActivateSessionReq { game_id, signed_tx_b64: &b64 })
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
        .json(&SignReq { game_id, tx_b64: &b64 })
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
        return Err(format!("vps session_status: session not found for game {game_id}"));
    }
    if !resp.status().is_success() {
        return Err(format!("vps session_status: server error {}", resp.status()));
    }
    resp.json::<SessionStatus>()
        .map_err(|e| format!("vps session_status parse: {e}"))
}

/// Ask the user to sign the TEE auth message and forward it to the VPS to enable privacy.
#[cfg(feature = "solana")]
pub fn tee_authenticate(game_id: u64, wallet_pubkey: &str) -> Result<String, String> {
    use crate::multiplayer::solana::tauri_signer;
    use base64::Engine;
    use bevy::prelude::info;

    info!("[TEE-AUTH] Triggering TEE handshake for game {}", game_id);

    let sig_bytes = tauri_signer::sign_message_via_tauri(TEE_AUTH_MESSAGE)
        .map_err(|e| format!("TEE sign_message: {e}"))?;

    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&sig_bytes);

    let response = client()?
        .post(format!("{}/session/tee_auth", vps_base()))
        .json(&TeeAuthReq {
            game_id,
            wallet_pubkey,
            signature_b64: &sig_b64,
        })
        .send()
        .map_err(|e| format!("vps tee_auth: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps tee_auth: HTTP {status} — {body}"));
    }

    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps tee_auth parse: {e}"))?;

    info!("[TEE-AUTH] SUCCESS for game {} (TEE: {})", game_id, TEE_DEVNET_ADDR);
    Ok(resp.sig)
}
