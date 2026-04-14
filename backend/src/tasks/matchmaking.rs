//! Matchmaking service for the XFChess backend.
//!
//! This module provides ELO-based player pairing for multiplayer games.

use crate::signing::routes::matchmaking::{SharedMatchmakingState, MatchResult};
use tracing::info;
use std::collections::HashSet;

/// Runs the matchmaking service.
///
/// This service runs every 5 seconds and pairs players within 200 ELO points.
///
/// # Arguments
/// * `state` - The shared matchmaking state
pub async fn run_matchmaking_service(state: SharedMatchmakingState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    
    loop {
        interval.tick().await;
        
        let mut queue = state.queue.lock().unwrap();
        if queue.len() < 2 { continue; }
        
        // Sort by ELO
        queue.sort_by(|a, b| a.elo.cmp(&b.elo));
        
        let mut matched_indices = HashSet::new();
        let mut new_matches = vec![];
        
        for i in 0..queue.len() {
            if matched_indices.contains(&i) { continue; }
            for j in (i + 1)..queue.len() {
                if matched_indices.contains(&j) { continue; }
                
                let diff = queue[i].elo.abs_diff(queue[j].elo);
                if diff <= 200 { // Configurable tolerance
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
            let mut matches = state.matches.lock().unwrap();
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
