use axum::{
    routing::{get, post},
    Router,
};

#[path = "matchmaking/handlers.rs"]
pub mod handlers;
#[path = "matchmaking/state.rs"]
pub mod state;

// Re-export for existing routes::matchmaking callers.
pub use handlers::{join, leave, status, JoinRequest, LeaveRequest};
pub use state::{MatchResult, MatchmakingTicket, SharedMatchmakingState};

pub fn matchmaking_routes() -> Router<crate::signing::AppState> {
    Router::new()
        .route("/join", post(join))
        .route("/status/{pubkey}", get(status))
        .route("/leave", post(leave))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_matchmaking_ticket_serialization() -> anyhow::Result<()> {
        let ticket = MatchmakingTicket {
            pubkey: "test_wallet".to_string(),
            elo: 1500,
            joined_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System time should be after UNIX_EPOCH")?
                .as_secs(),
        };
        assert!(serde_json::to_string(&ticket).is_ok());
        Ok(())
    }

    #[test]
    fn test_match_result_serialization() -> anyhow::Result<()> {
        let result = MatchResult {
            game_id: 12345,
            opponent: "opponent_wallet".to_string(),
            is_white: true,
            matched_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("System time should be after UNIX_EPOCH")?
                .as_secs(),
        };
        assert!(serde_json::to_string(&result).is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_matchmaking_routes_creation() {
        let _router = matchmaking_routes();
    }

    #[tokio::test]
    async fn test_shared_matchmaking_state_default() -> anyhow::Result<()> {
        // Default() builds a lazy (unconnected) SQLite pool, which needs a
        // Tokio context to construct even though it doesn't connect eagerly.
        let state = SharedMatchmakingState::test_state()?;
        assert_eq!(state.queue.lock().len(), 0);
        assert_eq!(state.matches.lock().len(), 0);
        Ok(())
    }
}
