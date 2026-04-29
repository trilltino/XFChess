#![allow(dead_code)]
/// HTTP client for the XFChess signing-server VPS.
///
/// All calls are blocking (intended for use inside IoTaskPool async tasks).
use serde::{Deserialize, Serialize};
use bevy::prelude::info;

const VPS_DEFAULT_URL: &str = "http://127.0.0.1:8090";

fn vps_base() -> String {
    // Priority: runtime env var > compile-time BACKEND_URL > default
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| VPS_DEFAULT_URL.to_string())
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert("ngrok-skip-browser-warning", reqwest::header::HeaderValue::from_static("true"));
            h
        })
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

const TEE_AUTH_MESSAGE: &str = "Authenticate with MagicBlock TEE";
const TEE_DEVNET_ADDR: &str = "FnE6VJT5QNZdedZPnCoLsARgBwoE6DeJNjBs2H1gySXA";

// ── Request / Response types ──────────────────────────────────────────────────

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
struct RecordMoveReq<'a> {
    game_id: u64,
    move_uci: &'a str,
    next_fen: &'a str,
    nonce: u64,
}

#[derive(Serialize)]
struct SignReq<'a> {
    game_id: u64,
    tx_b64: &'a str,
}

#[derive(Serialize)]
struct UndelegateGameReq {
    game_id: u64,
}

#[derive(Serialize)]
struct FinalizeGameReq<'a> {
    game_id: u64,
    winner: Option<&'a str>,    // "white" | "black" | null
    white_pubkey: &'a str,
    black_pubkey: &'a str,
}

#[derive(Deserialize)]
struct SigResp {
    sig: String,
}

#[derive(Deserialize)]
pub struct SessionStatus {
    pub active: bool,
    pub session_pubkey: String,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Ask VPS to create (or return existing) session keypair for `game_id`.
/// Returns the session pubkey (base58).
pub fn create_session(game_id: u64, wallet_pubkey: &str) -> Result<String, String> {
    let resp = client()
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
/// `signed_tx_bytes` is the raw bincode-serialised `Transaction`.
pub fn activate_session(game_id: u64, signed_tx_bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(signed_tx_bytes);
    let response = client()
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

/// Ask VPS to build, sign, and submit a `record_move` instruction on the ER.
pub fn record_move(game_id: u64, move_uci: &str, next_fen: &str, nonce: u64) -> Result<String, String> {
    let response = client()
        .post(format!("{}/move/record", vps_base()))
        .json(&RecordMoveReq { game_id, move_uci, next_fen, nonce })
        .send()
        .map_err(|e| format!("vps record_move: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps record_move: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps record_move parse: {e}"))?;
    Ok(resp.sig)
}

/// Ask VPS to sign a pre-built TX with the session key and submit it.
/// Used for delegation: client builds the complex instruction, VPS signs.
pub fn sign_and_submit(game_id: u64, tx_bytes: &[u8]) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(tx_bytes);
    let resp = client()
        .post(format!("{}/session/sign", vps_base()))
        .json(&SignReq { game_id, tx_b64: &b64 })
        .send()
        .map_err(|e| format!("vps sign_and_submit: {e}"))?
        .json::<SigResp>()
        .map_err(|e| format!("vps sign_and_submit parse: {e}"))?;
    Ok(resp.sig)
}

/// Ask VPS to commit ER state back to devnet by submitting `undelegate_game` on the ER.
pub fn vps_undelegate_game(game_id: u64) -> Result<String, String> {
    let response = client()
        .post(format!("{}/game/undelegate", vps_base()))
        .json(&UndelegateGameReq { game_id })
        .send()
        .map_err(|e| format!("vps undelegate_game: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps undelegate_game: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps undelegate_game parse: {e}"))?;
    Ok(resp.sig)
}

/// Ask VPS to finalize the game on devnet (set Finished, pay wager, update ELO).
/// Must be called after `vps_undelegate_game` has committed the ER state.
pub fn vps_finalize_game(
    game_id: u64,
    winner: Option<&str>,
    white_pubkey: &str,
    black_pubkey: &str,
) -> Result<String, String> {
    let response = client()
        .post(format!("{}/game/finalize", vps_base()))
        .json(&FinalizeGameReq { game_id, winner, white_pubkey, black_pubkey })
        .send()
        .map_err(|e| format!("vps finalize_game: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps finalize_game: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps finalize_game parse: {e}"))?;
    Ok(resp.sig)
}

/// Query session status from VPS.
pub fn session_status(game_id: u64) -> Result<SessionStatus, String> {
    let resp = client()
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlayerProfile {
    pub elo: u32,
    pub country: String,
    pub username: String,
}

/// Fetch player profile details (ELO, country, username) from VPS.
pub fn fetch_player_profile(pubkey: &str) -> Result<PlayerProfile, String> {
    let resp = client()
        .get(format!("{}/player/{}", vps_base(), pubkey))
        .send()
        .map_err(|e| format!("vps fetch_player_profile: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps fetch_player_profile: HTTP {}", resp.status()));
    }
    resp.json::<PlayerProfile>()
        .map_err(|e| format!("vps fetch_player_profile parse: {e}"))
}

#[derive(Serialize)]
struct TeeAuthReq<'a> {
    game_id: u64,
    wallet_pubkey: &'a str,
    signature_b64: &'a str,
}

/// Ask the user to sign the TEE auth message and forward it to the VPS to enable privacy.
#[cfg(feature = "solana")]
pub fn tee_authenticate(game_id: u64, wallet_pubkey: &str) -> Result<String, String> {
    use crate::multiplayer::solana::tauri_signer;
    use base64::Engine;

    info!("[TEE-AUTH] Triggering TEE handshake for game {}", game_id);
    
    // 1. Sign the required message via Tauri signing bridge
    let sig_bytes = tauri_signer::sign_message_via_tauri(TEE_AUTH_MESSAGE)
        .map_err(|e| format!("TEE sign_message: {e}"))?;
    
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&sig_bytes);

    // 2. Forward the authentication to the VPS
    let response = client()
        .post(format!("{}/session/tee_auth", vps_base()))
        .json(&TeeAuthReq { 
            game_id, 
            wallet_pubkey, 
            signature_b64: &sig_b64 
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

// ── User verification status ────────────────────────────────────────────────
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

/// Register user identity and KYC data securely in the VPS vault.
pub fn register_identity(payload: &IdentityPayload) -> Result<(), String> {
    let response = client()
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

#[derive(Serialize)]
pub struct RegisterReq {
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
    pub username: String,
}

/// Register a wallet with a username in the backend.
pub fn register_wallet(req: &RegisterReq) -> Result<(), String> {
    let response = client()
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

#[derive(Serialize)]
pub struct LinkWalletReq {
    pub email: String,
    pub password: String,
    pub wallet: String,
    pub signature: String,
    pub timestamp: u64,
}

/// Link a wallet to an email-based account.
pub fn link_wallet(req: &LinkWalletReq) -> Result<(), String> {
    let response = client()
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

/// Fetch verification status for a wallet. Returns defaults on network error so
/// callers can decide whether to block hard or degrade gracefully.
pub fn get_user_status(wallet_pubkey: &str) -> Result<UserStatus, String> {
    let resp = client()
        .get(format!("{}/api/user/status/{}", vps_base(), wallet_pubkey))
        .send()
        .map_err(|e| format!("vps get_user_status: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps get_user_status: HTTP {}", resp.status()));
    }
    resp.json::<UserStatus>()
        .map_err(|e| format!("vps get_user_status parse: {e}"))
}

/// Async version of get_user_status — wraps the blocking call in spawn_blocking
/// so it can be awaited from a tokio task without blocking the async executor.
pub async fn get_user_status_async(wallet_pubkey: String) -> Result<UserStatus, String> {
    tokio::task::spawn_blocking(move || get_user_status(&wallet_pubkey))
        .await
        .map_err(|e| format!("vps get_user_status_async join: {e}"))?
}

/// Gate wagered-play entry: returns Ok(()) when the wallet may enter a wagered
/// match or cash tournament, otherwise a human-readable reason.
pub fn require_wager_eligibility(wallet_pubkey: &str) -> Result<(), String> {
    let status = get_user_status(wallet_pubkey)?;
    if status.can_wager {
        return Ok(());
    }
    let mut missing = Vec::new();
    if !status.has_profile {
        missing.push("profile");
    }
    if !status.has_email {
        missing.push("email");
    }
    if !status.has_kyc {
        missing.push("KYC");
    }
    Err(format!(
        "Wagered play blocked. Missing: {}. Complete setup on your Profile page.",
        missing.join(", ")
    ))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TournamentSummary {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee_lamports: u64,
    pub prize_pool: u64,
    pub registered: usize,
    pub status: String,
    pub is_private: bool,
}

pub fn list_tournaments() -> Result<Vec<TournamentSummary>, String> {
    let resp = client()
        .get(format!("{}/tournaments", vps_base()))
        .send()
        .map_err(|e| format!("vps list_tournaments: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("vps list_tournaments: HTTP {}", resp.status()));
    }
    resp.json::<Vec<TournamentSummary>>()
        .map_err(|e| format!("vps list_tournaments parse: {e}"))
}

pub fn join_tournament(tournament_id: u64, player_pubkey: &str, password: Option<&str>) -> Result<u32, String> {
    let mut body = serde_json::json!({
        "player": player_pubkey,
        "elo": 1200
    });
    if let Some(pw) = password {
        body["password"] = serde_json::Value::String(pw.to_string());
    }
    let resp = client()
        .post(format!("{}/tournament/{}/join", vps_base(), tournament_id))
        .json(&body)
        .send()
        .map_err(|e| format!("vps join_tournament: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps join_tournament: HTTP {status} - {body}"));
    }
    let data = resp.json::<serde_json::Value>()
        .map_err(|e| format!("vps join_tournament parse: {e}"))?;
    data.get("slot")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| "Missing slot in join response".to_string())
}

// ── P2P Relay API ────────────────────────────────────────────────────────────

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
}

#[derive(Serialize)]
struct P2PJoinReq<'a> {
    game_id: String,
    joiner_node_id: &'a str,
}

#[derive(Serialize)]
struct P2PMessageReq<'a> {
    game_id: String,
    from_node_id: &'a str,
    message: &'a str,
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
}

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

/// Announce a P2P game to the VPS relay
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
    let resp = client()
        .post(format!("{}/p2p/announce", vps_base()))
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

/// List available P2P games from VPS relay
pub fn p2p_list_games() -> Result<Vec<P2PGameListing>, String> {
    let resp = client()
        .get(format!("{}/p2p/games", vps_base()))
        .send()
        .map_err(|e| format!("vps p2p_list_games: {e}"))?;
    
    if !resp.status().is_success() {
        return Err(format!("vps p2p_list_games: HTTP {}", resp.status()));
    }
    
    resp.json::<Vec<P2PGameListing>>()
        .map_err(|e| format!("vps p2p_list_games parse: {e}"))
}

/// Join a P2P game via VPS relay
pub fn p2p_join_game(game_id: String, joiner_node_id: &str) -> Result<Option<String>, String> {
    let resp = client()
        .post(format!("{}/p2p/join", vps_base()))
        .json(&P2PJoinReq {
            game_id,
            joiner_node_id,
        })
        .send()
        .map_err(|e| format!("vps p2p_join: {e}"))?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_join: HTTP {status} - {body}"));
    }
    
    let result = resp.json::<P2PJoinResp>()
        .map_err(|e| format!("vps p2p_join parse: {e}"))?;
    
    if result.success {
        Ok(result.host_node_id)
    } else {
        Err("Join request rejected".to_string())
    }
}

/// Send a P2P message via VPS relay
pub fn p2p_send_message(game_id: String, from_node_id: &str, message: &str) -> Result<(), String> {
    let resp = client()
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
    
    Ok(())
}

/// Poll for P2P messages from VPS relay
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
    
    let resp = client()
        .post(format!("{}/p2p/poll", vps_base()))
        .json(&body)
        .send()
        .map_err(|e| format!("vps p2p_poll: {e}"))?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("vps p2p_poll: HTTP {status} - {body}"));
    }
    
    let result = resp.json::<P2PPollResp>()
        .map_err(|e| format!("vps p2p_poll parse: {e}"))?;
    
    Ok((result.messages, result.next_index))
}
