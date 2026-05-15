//! Game lifecycle endpoints on the VPS.
//!
//! Covers move recording on the Execution Rollup, committing ER state back
//! to devnet (`undelegate`), and finalizing games on-chain (winner payout,
//! ELO updates, cleanup).

use serde::Serialize;

use super::client::{client, vps_base};
use super::session::SigResp;

#[derive(Serialize)]
struct RecordMoveReq<'a> {
    game_id: u64,
    move_uci: &'a str,
    next_fen: &'a str,
    nonce: u64,
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
}

/// Ask VPS to build, sign, and submit a `record_move` instruction on the ER.
pub fn record_move(game_id: u64, move_uci: &str, next_fen: &str, nonce: u64) -> Result<String, String> {
    let response = client()?
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

/// Ask VPS to commit ER state back to devnet by submitting `undelegate_game` on the ER.
pub fn vps_undelegate_game(game_id: u64) -> Result<String, String> {
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
    let response = client()?
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
