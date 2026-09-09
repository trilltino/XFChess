use serde::{Deserialize, Serialize};

use super::client::{client, vps_base};
use super::session::SigResp;

#[derive(Serialize)]
struct RecordMoveReq<'a> {
    game_id: u64,
    move_uci: &'a str,
    next_fen: &'a str,
    nonce: u64,
    mover_wallet: &'a str,
}

#[derive(Serialize)]
struct DelegateGameReq {
    game_id: u64,
}

#[derive(Serialize)]
struct UndelegateGameReq {
    game_id: u64,
}

#[derive(Serialize)]
struct FinalizeGameReq<'a> {
    game_id: u64,
    winner: Option<&'a str>, // "white" | "black" | null
    white_pubkey: &'a str,
    black_pubkey: &'a str,
    wager_lamports: u64,
}

#[derive(Debug, Clone, Default)]
pub struct FinalizeResult {
    pub sig: String,
    pub winner_lamports: u64,
    pub country_fee: u64,
    pub operating_cost_lamports: u64,
    pub elo_fee: u64,
}

#[derive(Deserialize)]
struct FinalizeResp {
    pub sig: String,
    #[serde(default)]
    pub winner_lamports: u64,
    #[serde(default)]
    pub country_fee: u64,
    #[serde(default)]
    pub operating_cost_lamports: u64,
    #[serde(default)]
    pub elo_fee: u64,
}

#[derive(Serialize)]
struct FreeRatedResultReq<'a> {
    game_id: u64,
    winner: Option<&'a str>,
    white_pubkey: &'a str,
    black_pubkey: &'a str,
}

#[derive(Serialize)]
struct DisputeReq<'a> {
    game_id: u64,
    disputing_player: &'a str,
}

#[derive(Serialize)]
struct BlurTelemetryReq<'a> {
    game_id: u64,
    move_number: u32,
    color: &'a str,
    blurred: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    think_ms: Option<u32>,
}

pub fn report_blur(
    game_id: u64,
    move_number: u32,
    color: &str,
    blurred: bool,
    think_ms: Option<u32>,
) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/telemetry/blur", vps_base()))
        .json(&BlurTelemetryReq {
            game_id,
            move_number,
            color,
            blurred,
            think_ms,
        })
        .send()
        .map_err(|e| format!("vps report_blur: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("vps report_blur: HTTP {}", response.status()));
    }
    Ok(())
}

pub fn record_move(
    game_id: u64,
    move_uci: &str,
    next_fen: &str,
    nonce: u64,
    mover_wallet: &str,
) -> Result<(String, String), String> {
    let response = client()?
        .post(format!("{}/move/record", vps_base()))
        .json(&RecordMoveReq {
            game_id,
            move_uci,
            next_fen,
            nonce,
            mover_wallet,
        })
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
    Ok((resp.sig, resp.er_endpoint))
}

pub fn vps_delegate_game(game_id: u64) -> Result<String, String> {
    let response = client()?
        .post(format!("{}/game/delegate", vps_base()))
        .json(&DelegateGameReq { game_id })
        .send()
        .map_err(|e| format!("vps delegate_game: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps delegate_game: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("vps delegate_game parse: {e}"))?;
    Ok(resp.sig)
}

pub fn vps_undelegate_game(game_id: u64) -> Result<(String, String), String> {
    let response = client()?
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
    Ok((resp.sig, resp.er_endpoint))
}

pub fn vps_finalize_game(
    game_id: u64,
    winner: Option<&str>,
    white_pubkey: &str,
    black_pubkey: &str,
    wager_lamports: u64,
) -> Result<FinalizeResult, String> {
    let response = client()?
        .post(format!("{}/game/finalize", vps_base()))
        .json(&FinalizeGameReq {
            game_id,
            winner,
            white_pubkey,
            black_pubkey,
            wager_lamports,
        })
        .send()
        .map_err(|e| format!("vps finalize_game: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("vps finalize_game: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<FinalizeResp>()
        .map_err(|e| format!("vps finalize_game parse: {e}"))?;
    Ok(FinalizeResult {
        sig: resp.sig,
        winner_lamports: resp.winner_lamports,
        country_fee: resp.country_fee,
        operating_cost_lamports: resp.operating_cost_lamports,
        elo_fee: resp.elo_fee,
    })
}

pub fn vps_submit_free_rated_result(
    game_id: u64,
    winner: Option<&str>,
    white_pubkey: &str,
    black_pubkey: &str,
) -> Result<(), String> {
    let response = client()?
        .post(format!("{}/ratings/update", vps_base()))
        .json(&FreeRatedResultReq {
            game_id,
            winner,
            white_pubkey,
            black_pubkey,
        })
        .send()
        .map_err(|e| format!("ratings/update: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("ratings/update: HTTP {status} — {body}"));
    }
    Ok(())
}

pub fn vps_fetch_move_nonce(game_id: u64) -> Result<u64, String> {
    #[derive(Deserialize)]
    struct NonceResp {
        nonce: u64,
    }
    let response = client()?
        .get(format!("{}/game/{}/nonce", vps_base(), game_id))
        .send()
        .map_err(|e| format!("fetch_nonce: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("fetch_nonce: HTTP {status}"));
    }
    let resp = response
        .json::<NonceResp>()
        .map_err(|e| format!("fetch_nonce parse: {e}"))?;
    // on-chain stores the last used nonce; next valid nonce is nonce + 1
    Ok(resp.nonce + 1)
}

pub fn get_active_game_for_wallet(wallet_pubkey: &str) -> Result<Option<u64>, String> {
    #[derive(Deserialize)]
    struct ActiveGameResp {
        game_id: Option<u64>,
    }
    let response = client()?
        .get(format!("{}/games/active/{}", vps_base(), wallet_pubkey))
        .send()
        .map_err(|e| format!("get_active_game: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Ok(None);
    }
    let resp = response
        .json::<ActiveGameResp>()
        .map_err(|e| format!("get_active_game parse: {e}"))?;
    Ok(resp.game_id)
}

pub fn fetch_verified_participants(game_id: u64) -> Result<Option<(String, String)>, String> {
    #[derive(Deserialize)]
    struct ParticipantsResp {
        white: String,
        black: String,
    }
    let response = client()?
        .get(format!("{}/game/{}/participants", vps_base(), game_id))
        .send()
        .map_err(|e| format!("fetch_verified_participants: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(format!(
            "fetch_verified_participants: HTTP {}",
            response.status()
        ));
    }
    let resp = response
        .json::<ParticipantsResp>()
        .map_err(|e| format!("fetch_verified_participants parse: {e}"))?;
    Ok(Some((resp.white, resp.black)))
}

pub fn get_game_moves_for_spectator(game_id: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct MoveEntry {
        move_uci: String,
    }
    #[derive(Deserialize)]
    struct MovesResp {
        moves: Vec<MoveEntry>,
    }

    let response = client()?
        .get(format!("{}/games/moves/{}", vps_base(), game_id))
        .send()
        .map_err(|e| format!("spectator get_moves: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("spectator get_moves: HTTP {status}"));
    }
    let resp = response
        .json::<MovesResp>()
        .map_err(|e| format!("spectator get_moves parse: {e}"))?;
    Ok(resp.moves.into_iter().map(|m| m.move_uci).collect())
}

pub fn get_broadcast_delay(game_id: &str) -> Result<u64, String> {
    #[derive(Deserialize)]
    struct DelayResp {
        delay_secs: i64,
    }

    let response = client()?
        .get(format!("{}/games/{}/broadcast-delay", vps_base(), game_id))
        .send()
        .map_err(|e| format!("spectator get_broadcast_delay: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "spectator get_broadcast_delay: HTTP {}",
            response.status()
        ));
    }
    let resp = response
        .json::<DelayResp>()
        .map_err(|e| format!("spectator get_broadcast_delay parse: {e}"))?;
    Ok(resp.delay_secs.max(0) as u64)
}

pub fn fetch_move_log(game_id: u64) -> Result<Vec<braid_chess::MovePayload>, String> {
    // Path and response shape must match `game_log.rs`'s registered route
    // exactly: `GET /game/{id}/moves` (singular "game"), returning a bare
    // JSON array of `ChessMessage`s (the plain-GET snapshot path, see
    // `GameLogState::snapshot` — not the `{"moves": [...]}` envelope this
    // used to assume, which never matched either the URL or the body shape
    // the backend actually serves and made this 404 on every call).
    let response = client()?
        .get(format!("{}/game/{}/moves", vps_base(), game_id))
        .send()
        .map_err(|e| format!("fetch_move_log: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("fetch_move_log: HTTP {status}"));
    }
    let messages = response
        .json::<Vec<braid_chess::ChessMessage>>()
        .map_err(|e| format!("fetch_move_log parse: {e}"))?;
    Ok(messages
        .into_iter()
        .filter_map(|m| match m {
            braid_chess::ChessMessage::Move(payload) => Some(payload),
            _ => None,
        })
        .collect())
}

pub fn vps_submit_dispute(game_id: u64, disputing_player: &str) -> Result<String, String> {
    let response = client()?
        .post(format!("{}/dispute/submit", vps_base()))
        .json(&DisputeReq {
            game_id,
            disputing_player,
        })
        .send()
        .map_err(|e| format!("dispute/submit: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("dispute/submit: HTTP {status} — {body}"));
    }
    let resp = response
        .json::<SigResp>()
        .map_err(|e| format!("dispute/submit parse: {e}"))?;
    Ok(resp.sig)
}
