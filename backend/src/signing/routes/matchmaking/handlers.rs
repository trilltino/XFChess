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
    State(app_state): State<crate::signing::AppState>,
    Json(req): Json<JoinRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let state = &app_state.matchmaking;
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

    // Determine effective ELO for matchmaking.
    // If the player was seeded from external and has a linked Lichess account,
    // the on-chain elo_rating already reflects that seed. For brand-new players
    // whose on-chain profile may not have synced yet, fall back to the backend DB.
    let mut effective_elo = cached_elo.elo_rating;
    if cached_elo.seeded_from_external {
        info!("[Matchmaking] Player {} using externally-seeded ELO {}", req.pubkey, cached_elo.elo_rating);
    } else if effective_elo == 120000.0 {
        // Still at default — check backend DB for a pending link
        let pool = app_state.store.pool();
        if let Ok(row) = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT blitz_rating, rapid_rating, bullet_rating FROM external_elo_links WHERE pubkey = ? AND platform = 'lichess'"
        )
        .bind(&req.pubkey)
        .fetch_one(&pool)
        .await {
            let (blitz, rapid, bullet) = row;
            let best = [blitz, rapid, bullet].iter().copied().max().unwrap_or(0) as f64;
            if best > 0.0 {
                effective_elo = best * 100.0; // Convert to centiscale
                info!("[Matchmaking] Player {} using backend Lichess ELO {} (on-chain still default)", req.pubkey, best);
            }
        }
    }

    let ticket = MatchmakingTicket {
        pubkey: req.pubkey.clone(),
        elo: effective_elo as u32,
        joined_at: now,
    };

    let mut queue = state.queue.lock().expect("Mutex lock should not be poisoned");
    // Remove if already in queue to prevent duplicates
    queue.retain(|t| t.pubkey != req.pubkey);
    queue.push(ticket);

    info!(
        "[Matchmaking] Player {} joined queue with ELO {} (country: {})",
        req.pubkey, effective_elo, cached_elo.country
    );

    Ok(Json(()))
}

/// Handles `GET /matchmaking/status/{pubkey}` — checks if player has a match.
pub async fn status(
    State(app_state): State<crate::signing::AppState>,
    Path(pubkey): Path<String>,
) -> Result<Json<Option<MatchResult>>, (StatusCode, String)> {
    let state = &app_state.matchmaking;
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
    State(app_state): State<crate::signing::AppState>,
    Json(req): Json<LeaveRequest>,
) -> Result<Json<()>, (StatusCode, String)> {
    let state = &app_state.matchmaking;
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
