//! Shared data types and state for the matchmaking system.
//!
//! [`MatchmakingTicket`] represents a player waiting in the queue,
//! [`MatchResult`] is the payload handed back once they are paired, and
//! [`SharedMatchmakingState`] bundles the queue, pending matches, and ELO
//! cache behind `Arc<Mutex<_>>` so Axum handlers can share them.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Matchmaking ticket representing a player waiting for a match.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchmakingTicket {
    /// Player's wallet public key.
    pub pubkey: String,
    /// Player's ELO rating.
    pub elo: u32,
    /// Unix timestamp when the player joined the queue.
    pub joined_at: u64,
}

/// Match result returned when a player is matched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchResult {
    /// The game ID for the matched game.
    pub game_id: u64,
    /// Opponent's wallet public key.
    pub opponent: String,
    /// Whether the player plays as white.
    pub is_white: bool,
}

/// Shared state for the matchmaking system.
///
/// Contains the player queue, pending match results, and the ELO cache
/// used to look up on-chain ratings.
#[derive(Clone)]
pub struct SharedMatchmakingState {
    /// Queue of players waiting for matches.
    pub queue: Arc<Mutex<Vec<MatchmakingTicket>>>,
    /// Map from pubkey to match result (one-time retrieval).
    pub matches: Arc<Mutex<HashMap<String, MatchResult>>>,
    /// ELO cache for fetching player ratings.
    pub elo_cache: Arc<crate::signing::EloCache>,
}

impl Default for SharedMatchmakingState {
    fn default() -> Self {
        let program_id = solana_sdk::pubkey::Pubkey::new_from_array([0u8; 32]);
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            matches: Arc::new(Mutex::new(HashMap::new())),
            elo_cache: Arc::new(crate::signing::EloCache::new(
                "https://api.devnet.solana.com".to_string(),
                std::time::Duration::from_secs(300),
                program_id,
            )),
        }
    }
}
