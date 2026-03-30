/// HTTP client for the XFChess signing-server VPS.
///
/// All calls are blocking (intended for use inside IoTaskPool async tasks).
use serde::{Deserialize, Serialize};

const VPS_DEFAULT_URL: &str = "https://unrejuvenated-philologically-trudi.ngrok-free.dev";

fn vps_base() -> String {
    std::env::var("SIGNING_SERVICE_URL").unwrap_or_else(|_| VPS_DEFAULT_URL.to_string())
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
