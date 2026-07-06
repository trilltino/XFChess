//! Game lifecycle endpoints on the VPS.
//!
//! Covers move recording on the Execution Rollup, committing ER state back
//! to devnet (`undelegate`), and finalizing games on-chain (winner payout,
//! ELO updates, cleanup).

use serde::{Deserialize, Serialize};

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
    wager_lamports: u64,
}

/// Full finalization result returned by the VPS after `/game/finalize`.
#[derive(Debug, Clone, Default)]
pub struct FinalizeResult {
    /// On-chain transaction signature.
    pub sig: String,
    /// Lamports sent to the winner (0 for free games).
    pub winner_lamports: u64,
    /// Country/treasury fee deducted in lamports.
    pub country_fee: u64,
}

#[derive(Deserialize)]
struct FinalizeResp {
    pub sig: String,
    #[serde(default)]
    pub winner_lamports: u64,
    #[serde(default)]
    pub country_fee: u64,
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

/// Report a move's anti-cheat telemetry: whether the window lost focus since
/// this player's previous move (the alt-tab signature) and how long the move
/// took (`think_ms`). Fire-and-forget — failures are the caller's to log,
/// never to surface to the player.
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

/// Ask VPS to build, sign, and submit a `record_move` instruction on the ER.
pub fn record_move(
    game_id: u64,
    move_uci: &str,
    next_fen: &str,
    nonce: u64,
) -> Result<String, String> {
    let response = client()?
        .post(format!("{}/move/record", vps_base()))
        .json(&RecordMoveReq {
            game_id,
            move_uci,
            next_fen,
            nonce,
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
/// Returns full [`FinalizeResult`] including winner payout and fee amounts.
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
    })
}

/// Submit the result of a Free Rated (no-wager) game so the backend updates ELO
/// without requiring an on-chain finalize. Fires-and-forgets on the VPS side.
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

/// Fetch the current `move_log.nonce` from the VPS (which reads the on-chain MoveLog PDA).
/// Returns the *next* nonce to use (on-chain stored nonce + 1).
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

/// Check if the wallet has an active (in-progress) game on the backend.
/// Returns `Some(game_id)` if found, `None` if not or on error.
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

/// Fetch the full move list for a game (used by spectator mode).
/// Returns a list of UCI strings in order.
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

/// Fetch a game's public broadcast delay in seconds (0 = live). A spectator
/// queries this before subscribing to the live P2P gossip feed: a non-zero
/// delay means the only permitted public source is the delay-gated HTTP feed.
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

/// Fetch the full move log for a game as typed [`braid_chess::MovePayload`] values.
///
/// Used by Braid reconnection recovery: the caller filters the returned list
/// to find moves that arrived after a given `since_version` hash.
pub fn fetch_move_log(game_id: u64) -> Result<Vec<braid_chess::MovePayload>, String> {
    #[derive(serde::Deserialize)]
    struct MoveEntry {
        move_uci: String,
        fen_after: String,
        move_number: u32,
        player: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct MovesResp {
        moves: Vec<MoveEntry>,
    }

    let response = client()?
        .get(format!("{}/games/{}/moves", vps_base(), game_id))
        .send()
        .map_err(|e| format!("fetch_move_log: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("fetch_move_log: HTTP {status}"));
    }
    let resp = response
        .json::<MovesResp>()
        .map_err(|e| format!("fetch_move_log parse: {e}"))?;
    Ok(resp
        .moves
        .into_iter()
        .map(|m| {
            braid_chess::MovePayload::from_uci(
                m.move_uci,
                m.fen_after,
                m.move_number,
                m.player.unwrap_or_default(),
            )
        })
        .collect())
}

/// Submit a dispute for a completed wager game. The VPS builds and submits the
/// `dispute` on-chain instruction and opens a 48-hour arbitration window.
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
