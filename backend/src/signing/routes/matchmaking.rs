//! Matchmaking queue and matching system for XFChess.
//!
//! Provides an in-memory matchmaking queue that matches players by ELO
//! rating. Players join with a wallet signature and are periodically
//! matched against opponents with similar ratings. ELO ratings are
//! fetched from on-chain `PlayerProfile` accounts via the ELO cache.
//!
//! Submodules:
//! - [`state`] — data types and [`SharedMatchmakingState`]
//! - [`handlers`] — `/join`, `/status/{pubkey}`, `/leave` HTTP handlers

use axum::{
    routing::{get, post},
    Router,
};

#[path = "matchmaking/state.rs"]
pub mod state;
#[path = "matchmaking/handlers.rs"]
pub mod handlers;

// Re-exports so existing call sites (e.g. `routes::matchmaking::SharedMatchmakingState`,
// `routes::matchmaking::matchmaking_routes`) keep working unchanged.
pub use state::{MatchResult, MatchmakingTicket, SharedMatchmakingState};
pub use handlers::{JoinRequest, LeaveRequest, join, leave, status};

/// Creates the matchmaking routes router.
///
/// # Arguments
/// * `state` - Shared matchmaking state
///
/// # Returns
/// An Axum Router with matchmaking endpoints.
pub fn matchmaking_routes(state: SharedMatchmakingState) -> Router {
    Router::new()
        .route("/join", post(join))
        .route("/status/{pubkey}", get(status))
        .route("/leave", post(leave))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        assert!(serde_json::to_string(&ticket).is_ok());
    }

    #[test]
    fn test_match_result_serialization() {
        let result = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
        };
        assert!(serde_json::to_string(&result).is_ok());
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
}
