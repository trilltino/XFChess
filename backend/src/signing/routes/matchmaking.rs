//! Matchmaking queue and matching system for XFChess.
//!
//! This module provides an in-memory matchmaking queue that matches players
//! by ELO rating. Players join the queue with their wallet signature and
//! are periodically matched against opponents with similar ratings.
//!
//! ELO ratings are fetched from on-chain PlayerProfile accounts via the ELO cache,
//! eliminating the need for clients to know their current rating.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

/// Matchmaking ticket representing a player waiting for a match.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchmakingTicket {
    /// Player's wallet public key
    pub pubkey: String,
    /// Player's ELO rating
    pub elo: u32,
    /// Unix timestamp when the player joined the queue
    pub joined_at: u64,
}

/// Match result returned when a player is matched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchResult {
    /// The game ID for the matched game
    pub game_id: u64,
    /// Opponent's wallet public key
    pub opponent: String,
    /// Whether the player plays as white
    pub is_white: bool,
}

/// Shared state for the matchmaking system.
///
/// Contains the player queue and pending match results.
#[derive(Clone)]
pub struct SharedMatchmakingState {
    /// Queue of players waiting for matches
    pub queue: Arc<Mutex<Vec<MatchmakingTicket>>>,
    /// Map from pubkey to match result (one-time retrieval)
    pub matches: Arc<Mutex<HashMap<String, MatchResult>>>,
    /// ELO cache for fetching player ratings
    pub elo_cache: Arc<crate::signing::EloCache>,
}

impl Default for SharedMatchmakingState {
    fn default() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            matches: Arc::new(Mutex::new(HashMap::new())),
            elo_cache: Arc::new(crate::signing::EloCache::new(
                "https://api.devnet.solana.com".to_string(),
                std::time::Duration::from_secs(300),
                solana_sdk::pubkey::Pubkey::default(),
            )),
        }
    }
}

/// Creates the matchmaking routes router.
///
/// # Arguments
/// * `state` - Shared matchmaking state
///
/// # Returns
/// An Axum Router with matchmaking endpoints
pub fn matchmaking_routes(state: SharedMatchmakingState) -> Router {
    Router::new()
        .route("/join", post(join))
        .route("/status/{pubkey}", get(status))
        .route("/leave", post(leave))
        .with_state(state)
}

/// Request to join the matchmaking queue.
#[derive(Deserialize, Serialize)]
pub struct JoinRequest {
    /// Player's wallet public key
    pub pubkey: String,
    /// Signature over "join_matchmaking:<timestamp>"
    pub signature: String,
    /// Unix timestamp for replay protection
    pub timestamp: u64,
}

/// Handles POST /matchmaking/join - adds player to matchmaking queue.
///
/// # Arguments
/// * `State(state)` - Shared matchmaking state
/// * `Json(req)` - Join request
///
/// # Returns
/// Empty JSON on success, error tuple on failure
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
    let cached_elo = state.elo_cache.get_elo(&req.pubkey).await
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

    info!("[Matchmaking] Player {} joined queue with ELO {} (country: {})", 
        req.pubkey, cached_elo.elo_rating, cached_elo.country);

    Ok(Json(()))
}

/// Handles GET /matchmaking/status/{pubkey} - checks if player has a match.
///
/// # Arguments
/// * `State(state)` - Shared matchmaking state
/// * `Path(pubkey)` - Player's wallet public key
///
/// # Returns
/// Match result or error
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

/// Request to leave the matchmaking queue.
#[derive(Deserialize, Serialize)]
pub struct LeaveRequest {
    /// Player's wallet public key
    pub pubkey: String,
    /// Signature over "leave_matchmaking:<timestamp>"
    pub signature: String,
    /// Unix timestamp for replay protection
    pub timestamp: u64,
}

/// Handles POST /matchmaking/leave - removes player from queue.
///
/// # Arguments
/// * `state` - Application state
/// * `req` - Leave request with pubkey, signature, timestamp
///
/// # Returns
/// Empty JSON on success, error tuple on failure
async fn leave(
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::Router,
    };
    use tower::ServiceExt;
    use std::time::SystemTime;

    #[test]
    fn test_matchmaking_ticket_serialization() {
        let ticket = MatchmakingTicket {
            pubkey: "test_wallet".to_string(),
            elo: 1500,
            joined_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time should be after UNIX_EPOCH")
                .as_secs(),
        };

        let json = serde_json::to_string(&ticket);
        assert!(json.is_ok());
    }

    #[test]
    fn test_match_result_serialization() {
        let result = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
        };

        let json = serde_json::to_string(&result);
        assert!(json.is_ok());
    }

    #[test]
    fn test_join_request_serialization() {
        let req = JoinRequest {
            pubkey: "test_wallet".to_string(),
            signature: "test_signature".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time should be after UNIX_EPOCH")
                .as_secs(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_leave_request_serialization() {
        let req = LeaveRequest {
            pubkey: "test_wallet".to_string(),
            signature: "test_signature".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time should be after UNIX_EPOCH")
                .as_secs(),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[tokio::test]
    async fn test_matchmaking_routes_creation() {
        let state = SharedMatchmakingState::default();
        let _router = matchmaking_routes(state);
    }

    #[test]
    fn test_shared_matchmaking_state_default() {
        let state = SharedMatchmakingState::default();
        assert_eq!(state.queue.lock().expect("Mutex lock").len(), 0);
        assert_eq!(state.matches.lock().expect("Mutex lock").len(), 0);
    }

    #[test]
    fn test_matchmaking_ticket_creation() {
        let ticket = MatchmakingTicket {
            pubkey: "test_wallet".to_string(),
            elo: 1500,
            joined_at: 1234567890,
        };

        assert_eq!(ticket.pubkey, "test_wallet");
        assert_eq!(ticket.elo, 1500);
        assert_eq!(ticket.joined_at, 1234567890);
    }

    #[test]
    fn test_match_result_creation() {
        let result = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
        };

        assert_eq!(result.game_id, 12345);
        assert_eq!(result.opponent, "opponent_wallet");
        assert!(result.is_white);
    }

    #[test]
    fn test_elo_range_validation() {
        // Test reasonable ELO ranges
        let valid_elos = vec![0, 1000, 1500, 2000, 3000];
        for elo in valid_elos {
            let ticket = MatchmakingTicket {
                pubkey: "test_wallet".to_string(),
                elo,
                joined_at: 1234567890,
            };
            assert_eq!(ticket.elo, elo);
        }
    }

    #[test]
    fn test_pubkey_format() {
        // Test that pubkey is a non-empty string
        let ticket = MatchmakingTicket {
            pubkey: "test_wallet".to_string(),
            elo: 1500,
            joined_at: 1234567890,
        };
        assert!(!ticket.pubkey.is_empty());
    }

    #[test]
    fn test_timestamp_validation() {
        // Test that timestamps are reasonable
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time should be after UNIX_EPOCH")
            .as_secs();

        let ticket = MatchmakingTicket {
            pubkey: "test_wallet".to_string(),
            elo: 1500,
            joined_at: now,
        };
        assert!(ticket.joined_at > 0);
    }

    #[test]
    fn test_queue_operations() {
        let state = SharedMatchmakingState::default();
        let mut queue = state.queue.lock().expect("Mutex lock");

        // Test adding tickets
        let ticket1 = MatchmakingTicket {
            pubkey: "player1".to_string(),
            elo: 1500,
            joined_at: 1234567890,
        };
        queue.push(ticket1);
        assert_eq!(queue.len(), 1);

        // Test removing tickets
        let ticket2 = MatchmakingTicket {
            pubkey: "player2".to_string(),
            elo: 1600,
            joined_at: 1234567891,
        };
        queue.push(ticket2);
        assert_eq!(queue.len(), 2);

        queue.retain(|t| t.pubkey != "player1");
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_match_operations() {
        let state = SharedMatchmakingState::default();
        let mut matches = state.matches.lock().expect("Mutex lock");

        // Test adding match results
        let result = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
        };
        matches.insert("player1".to_string(), result);
        assert_eq!(matches.len(), 1);

        // Test retrieving match results
        let retrieved = matches.get("player1");
        assert!(retrieved.is_some());

        // Test removing match results
        matches.remove("player1");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_is_white_boolean() {
        // Test both true and false values
        let result_white = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
        };
        assert!(result_white.is_white);

        let result_black = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: false,
        };
        assert!(!result_black.is_white);
    }
}
