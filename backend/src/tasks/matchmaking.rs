//! Matchmaking service for the XFChess backend.
//!
//! This module provides ELO-based player pairing for multiplayer games.

use crate::signing::routes::matchmaking::{SharedMatchmakingState, MatchResult};
use tracing::error;
use tracing::info;
use std::collections::HashSet;

/// Matchmaking service runs every interval duration (seconds)
pub const MATCHMAKING_INTERVAL_SECONDS: u64 = 5;

/// Maximum ELO difference for pairing players
pub const MATCHMAKING_ELO_TOLERANCE: u32 = 200;

/// Minimum players required to form a match
pub const MATCHMAKING_MIN_PLAYERS: usize = 2;

/// Runs the matchmaking service.
///
/// This service runs every MATCHMAKING_INTERVAL_SECONDS and pairs players within MATCHMAKING_ELO_TOLERANCE.
///
/// # Arguments
/// * `state` - The shared matchmaking state
pub async fn run_matchmaking_service(state: SharedMatchmakingState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(MATCHMAKING_INTERVAL_SECONDS));
    
    loop {
        interval.tick().await;
        
        let mut queue = match state.queue.lock() {
            Ok(q) => q,
            Err(e) => {
                error!("[MATCHMAKING] Queue mutex poisoned: {}", e);
                continue;
            }
        };
        if queue.len() < MATCHMAKING_MIN_PLAYERS { continue; }
        
        // Sort by ELO
        queue.sort_by(|a, b| a.elo.cmp(&b.elo));
        
        let mut matched_indices = HashSet::new();
        let mut new_matches = vec![];
        
        for i in 0..queue.len() {
            if matched_indices.contains(&i) { continue; }
            for j in (i + 1)..queue.len() {
                if matched_indices.contains(&j) { continue; }
                
                let diff = queue[i].elo.abs_diff(queue[j].elo);
                if diff <= MATCHMAKING_ELO_TOLERANCE { // Configurable tolerance
                    let game_id = rand::random::<u64>();
                    new_matches.push((queue[i].clone(), queue[j].clone(), game_id));
                    matched_indices.insert(i);
                    matched_indices.insert(j);
                    break;
                }
            }
        }
        
        // Rebuild queue without matched players
        let new_queue = queue.iter().enumerate()
            .filter(|(idx, _)| !matched_indices.contains(idx))
            .map(|(_, t)| t.clone())
            .collect();
        *queue = new_queue;
        
        // Save results to match map
        if !new_matches.is_empty() {
            let mut matches = match state.matches.lock() {
                Ok(m) => m,
                Err(e) => {
                    error!("[MATCHMAKING] Matches mutex poisoned: {}", e);
                    continue;
                }
            };
            for (p1, p2, game_id) in new_matches {
                info!("[Matchmaking] Paired {} vs {}", p1.pubkey, p2.pubkey);
                matches.insert(p1.pubkey.clone(), MatchResult {
                    game_id,
                    opponent: p2.pubkey.clone(),
                    is_white: true,
                });
                matches.insert(p2.pubkey.clone(), MatchResult {
                    game_id,
                    opponent: p1.pubkey.clone(),
                    is_white: false,
                });
            }
        }
    }
}
