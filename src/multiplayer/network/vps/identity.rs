//! Identity, profile, KYC, and wallet-linking endpoints on the VPS.
//!
//! Exposes helpers to:
//! - Fetch player profiles (ELO, country, username).
//! - Register encrypted identity / KYC data into the VPS vault.
//! - Register a wallet + username and link wallets to email accounts.
//! - Query the user's verification status and gate wagered-play entry.

use serde::{Deserialize, Serialize};

use super::client::{client, vps_base};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlayerProfile {
    pub elo: u32,
    pub country: String,
    pub username: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct UserStatus {
    pub has_profile: bool,
    pub has_email: bool,
    pub has_kyc: bool,
    pub can_wager: bool,
}

#[derive(Serialize)]
pub struct IdentityPayload {
    pub pubkey: String,
    pub full_name: String,
    pub dob: String,
    pub address: String,
    pub country: String,
    pub tax_id: String,
    pub signature: String,
    pub timestamp: u64,
    pub consent_kyc: bool,
    pub consent_retention_years: u8,
}

#[derive(Serialize)]
pub struct RegisterReq {
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
    pub username: String,
}

#[derive(Serialize)]
pub struct LinkWalletReq {
    pub email: String,
    pub password: String,
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
}

/// Fetch player profile details (ELO, country, username) from VPS.
pub fn fetch_player_profile(pubkey: &str) -> Result<PlayerProfile, String> {
    let resp = client()?
        .get(format!("{}/player/{}", vps_base(), pubkey))
        .send()
        .map_err(|e| format!("vps fetch_player_profile: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps fetch_player_profile: HTTP {}", resp.status()));
    }
    resp.json::<PlayerProfile>()
        .map_err(|e| format!("vps fetch_player_profile parse: {e}"))
}

/// Register user identity and KYC data securely in the VPS vault.
pub fn register_identity(payload: &IdentityPayload) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/identity/register", vps_base()))
        .json(payload)
        .send()
        .map_err(|e| format!("vps register_identity: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps register_identity: HTTP {status} — {body}"));
    }

    Ok(())
}

/// Register a wallet with a username in the backend.
pub fn register_wallet(req: &RegisterReq) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/api/auth/register", vps_base()))
        .json(req)
        .send()
        .map_err(|e| format!("vps register_wallet: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps register_wallet: HTTP {status} — {body}"));
    }

    Ok(())
}

/// Link a wallet to an email-based account.
pub fn link_wallet(req: &LinkWalletReq) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/api/auth/link-wallet", vps_base()))
        .json(req)
        .send()
        .map_err(|e| format!("vps link_wallet: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps link_wallet: HTTP {status} — {body}"));
    }

    Ok(())
}

/// Fetch verification status for a wallet.
///
/// Returns defaults on network error so callers can decide whether to block
/// hard or degrade gracefully.
pub fn get_user_status(wallet_pubkey: &str) -> Result<UserStatus, String> {
    let resp = client()?
        .get(format!("{}/api/user/status/{}", vps_base(), wallet_pubkey))
        .send()
        .map_err(|e| format!("vps get_user_status: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps get_user_status: HTTP {}", resp.status()));
    }
    resp.json::<UserStatus>()
        .map_err(|e| format!("vps get_user_status parse: {e}"))
}

/// Async wrapper around `get_user_status` — spawns the blocking call on a
/// dedicated thread so it can be awaited from a tokio task.
pub async fn get_user_status_async(wallet_pubkey: String) -> Result<UserStatus, String> {
    tokio::task::spawn_blocking(move || get_user_status(&wallet_pubkey))
        .await
        .map_err(|e| format!("vps get_user_status_async join: {e}"))?
}

/// Gate wagered-play entry: returns `Ok(())` when the wallet may enter a
/// wagered match or cash tournament, otherwise a human-readable reason.
pub fn require_wager_eligibility(wallet_pubkey: &str) -> Result<(), String> {
    let status = get_user_status(wallet_pubkey)?;
    if status.can_wager {
        return Ok(());
    }
    // Note: `has_email` is deliberately not checked here — the backend's
    // `can_wager` formula (backend/src/signing/routes/kyc.rs) never factors
    // in email, since email/JWT auth is cosmetic, not a gameplay gate. Listing
    // it as a blocking reason would tell the user to fix something that
    // doesn't affect eligibility.
    let mut missing = Vec::new();
    if !status.has_profile {
        missing.push("profile");
    }
    if !status.has_kyc {
        missing.push("KYC");
    }
    if missing.is_empty() {
        // can_wager=false but all sub-flags are set — account suspended or region-blocked
        return Err(
            "Wagered play is not available for this account. Visit your Profile page for details."
                .to_string(),
        );
    }
    Err(format!(
        "Wagered play blocked — missing: {}. Complete setup on your Profile page.",
        missing.join(", ")
    ))
}
