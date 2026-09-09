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

use super::state::{MatchResult, MatchmakingTicket};

#[derive(Deserialize, Serialize)]
pub struct JoinRequest {
    pub pubkey: String,
    pub signature: String,
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize)]
pub struct LeaveRequest {
    pub pubkey: String,
    pub signature: String,
    pub timestamp: u64,
}

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

    let bans = crate::db::repository::BanRepository::new(app_state.store.pool());
    if bans.is_banned(&req.pubkey).await.unwrap_or(false) {
        return Err((StatusCode::FORBIDDEN, "This wallet is banned.".to_string()));
    }

    // Fetch ELO from on-chain profile via cache
    let cached_elo = state.elo_cache.get_elo(&req.pubkey).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fetch ELO: {}", e),
        )
    })?;

    // Effective ELO for matchmaking. This is the Classical/default bucket —
    // the queue doesn't yet partition by time control (see `JoinRequest`),
    // so it can't select `elo_bullet`/`elo_blitz`/`elo_rapid` per the
    // requested game's pace; that's a follow-up to wire through once
    // matchmaking itself becomes time-control-aware. Lichess ratings are no
    // longer consulted here at all — `link_external_elo` no longer seeds
    // `elo_rating`, so a Lichess-only fallback would just be stale/unrelated
    // data standing in for a real (if still-default) XFChess rating.
    let effective_elo = cached_elo.elo_rating;

    let ticket = MatchmakingTicket {
        pubkey: req.pubkey.clone(),
        elo: effective_elo as u32,
        joined_at: now,
    };

    {
        let mut queue = state
            .queue
            .lock()
            .expect("Mutex lock should not be poisoned");
        // Remove if already in queue to prevent duplicates
        queue.retain(|t| t.pubkey != req.pubkey);
        queue.push(ticket.clone());
    }

    // Persist so a backend restart doesn't drop this ticket (migration 022).
    if let Err(e) = sqlx::query(
        "INSERT OR REPLACE INTO matchmaking_queue (pubkey, elo, joined_at) VALUES (?, ?, ?)",
    )
    .bind(&ticket.pubkey)
    .bind(ticket.elo as i64)
    .bind(ticket.joined_at as i64)
    .execute(&app_state.store.pool())
    .await
    {
        tracing::error!("[Matchmaking] Failed to persist queue ticket: {e}");
    }

    info!(
        "[Matchmaking] Player {} joined queue with ELO {} (country: {})",
        req.pubkey, effective_elo, cached_elo.country
    );

    Ok(Json(()))
}

pub async fn status(
    State(app_state): State<crate::signing::AppState>,
    Path(pubkey): Path<String>,
) -> Result<Json<Option<MatchResult>>, (StatusCode, String)> {
    let state = &app_state.matchmaking;
    let removed = {
        let mut matches = state
            .matches
            .lock()
            .expect("Mutex lock should not be poisoned");
        matches.remove(&pubkey)
    };

    if let Some(res) = removed {
        // One-time retrieval — clear the persisted copy too (migration 022).
        if let Err(e) = sqlx::query("DELETE FROM matchmaking_matches WHERE pubkey = ?")
            .bind(&pubkey)
            .execute(&app_state.store.pool())
            .await
        {
            tracing::error!("[Matchmaking] Failed to clear persisted match: {e}");
        }

        info!(
            "[Matchmaking] Player {} retrieved match {}",
            pubkey, res.game_id
        );
        Ok(Json(Some(res)))
    } else {
        Ok(Json(None))
    }
}

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

    {
        let mut queue = state
            .queue
            .lock()
            .expect("Mutex lock should not be poisoned");
        queue.retain(|t| t.pubkey != req.pubkey);
    }

    if let Err(e) = sqlx::query("DELETE FROM matchmaking_queue WHERE pubkey = ?")
        .bind(&req.pubkey)
        .execute(&app_state.store.pool())
        .await
    {
        tracing::error!("[Matchmaking] Failed to clear persisted queue ticket: {e}");
    }

    info!("[Matchmaking] Player {} left queue", req.pubkey);

    Ok(Json(()))
}
