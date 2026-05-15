//! HTTP handlers for the matchmaking queue.
//!
//! All endpoints verify a wallet signature over the message
//! `"<action>:<timestamp>"` (≤ 120s old) before mutating queue state,
//! and use the ELO cache to stamp the player's rating onto the ticket.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

use super::state::{MatchResult, MatchmakingTicket, SharedMatchmakingState};

/// Request to join the matchmaking queue.
#[derive(Deserialize, Serialize)]
pub struct JoinRequest {
    /// Player's wallet public key.
    pub pubkey: String,
    /// Signature over `"join_matchmaking:<timestamp>"`.
    pub signature: String,
    /// Unix timestamp for replay protection.
    pub timestamp: u64,
}

/// Request to leave the matchmaking queue.
#[derive(Deserialize, Serialize)]
pub struct LeaveRequest {
    /// Player's wallet public key.
    pub pubkey: String,
    /// Signature over `"leave_matchmaking:<timestamp>"`.
    pub signature: String,
    /// Unix timestamp for replay protection.
    pub timestamp: u64,
}

/// Handles `POST /matchmaking/join` — adds player to matchmaking queue.
pub async fn join(
    State(state): State<SharedMatchmakingState>,
    Json(req): Json<JoinRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let pk = Pubkey::from_str(&req.pubkey).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let sig = Signature::from_str(&req.signature)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let msg = format!("join_matchmaking:{}", req.timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Signature".to_string()));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after UNIX_EPOCH")
        .as_secs();

    if now > req.timestamp && now - req.timestamp > 120 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old".to_string()));
    }

    // Fetch ELO from on-chain profile via cache
    let cached_elo = state
        .elo_cache
        .get_elo(&req.pubkey)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch ELO: {}", e)))?;

    let ticket = MatchmakingTicket {
        pubkey: req.pubkey.clone(),
        elo: cached_elo.elo_rating as u32,
        joined_at: now,
    };

    let mut queue = state.queue.lock().expect("Mutex lock should not be poisoned");
    // Remove if already in queue to prevent duplicates
    queue.retain(|t| t.pubkey != req.pubkey);
    queue.push(ticket);

    info!(
        "[Matchmaking] Player {} joined queue with ELO {} (country: {})",
        req.pubkey, cached_elo.elo_rating, cached_elo.country
    );

    Ok(Json(()))
}

/// Handles `GET /matchmaking/status/{pubkey}` — checks if player has a match.
pub async fn status(
    State(state): State<SharedMatchmakingState>,
    Path(pubkey): Path<String>,
) -> Result<Json<Option<MatchResult>>, (StatusCode, String)> {
    let mut matches = state.matches.lock().expect("Mutex lock should not be poisoned");
    if let Some(res) = matches.remove(&pubkey) {
        info!("[Matchmaking] Player {} retrieved match {}", pubkey, res.game_id);
        Ok(Json(Some(res)))
    } else {
        Ok(Json(None))
    }
}

/// Handles `POST /matchmaking/leave` — removes player from queue.
pub async fn leave(
    State(state): State<SharedMatchmakingState>,
    Json(req): Json<LeaveRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let pk = Pubkey::from_str(&req.pubkey).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let sig = Signature::from_str(&req.signature)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let msg = format!("leave_matchmaking:{}", req.timestamp);
    if !sig.verify(pk.as_ref(), msg.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Signature".to_string()));
    }

    let mut queue = state.queue.lock().expect("Mutex lock should not be poisoned");
    queue.retain(|t| t.pubkey != req.pubkey);

    info!("[Matchmaking] Player {} left queue", req.pubkey);

    Ok(Json(()))
}
