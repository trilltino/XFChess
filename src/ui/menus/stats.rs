//! Platform statistics display in-game
//!
//! Fetches and displays global player/game counts from the VPS backend.

use bevy::prelude::*;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Resource, Default, Clone)]
pub struct PlatformStats {
    pub active_games: u64,
    pub unique_players: u64,
    pub total_sessions: u64,
    pub last_updated: f64,
}

#[derive(Resource, Clone)]
pub struct StatsFetchChannel {
    pub receiver: Arc<Mutex<std::sync::mpsc::Receiver<StatsResult>>>,
    pub sender: std::sync::mpsc::SyncSender<StatsResult>,
}

#[derive(Debug, Clone)]
pub struct StatsResult {
    pub active_games: u64,
    pub unique_players: u64,
    pub total_sessions: u64,
}

#[derive(Deserialize)]
struct StatsResponse {
    active_games: u64,
    unique_players: u64,
    total_sessions: u64,
}

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = std::sync::mpsc::sync_channel::<StatsResult>(4);
        app.insert_resource(StatsFetchChannel {
            receiver: Arc::new(Mutex::new(rx)),
            sender: tx,
        })
        .init_resource::<PlatformStats>()
        .add_systems(Update, fetch_stats_system);
    }
}

/// Dispatch a background thread to fetch stats (no blocking on game thread)
fn fetch_stats_system(
    mut stats: ResMut<PlatformStats>,
    channel: Res<StatsFetchChannel>,
    time: Res<Time>,
) {
    let now = time.elapsed().as_secs_f64();

    // Drain any completed fetch results
    if let Ok(rx) = channel.receiver.lock() {
        while let Ok(result) = rx.try_recv() {
            stats.active_games = result.active_games;
            stats.unique_players = result.unique_players;
            stats.total_sessions = result.total_sessions;
            stats.last_updated = now;
        }
    }

    // Only dispatch a new fetch every 30 seconds
    if now - stats.last_updated < 30.0 && stats.last_updated > 0.0 {
        return;
    }
    // Mark attempted so we don't re-dispatch every frame while waiting
    stats.last_updated = now;

    let vps_url = crate::multiplayer::network::vps::vps_base();
    let tx = channel.sender.clone();

    std::thread::spawn(move || {
        let url = format!("{}/stats", vps_url);
        match reqwest::blocking::get(&url) {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<StatsResponse>() {
                    let _ = tx.try_send(StatsResult {
                        active_games: data.active_games,
                        unique_players: data.unique_players,
                        total_sessions: data.total_sessions,
                    });
                }
            }
            Ok(resp) => trace!("Stats endpoint returned {}", resp.status()),
            Err(e) => trace!("Failed to fetch stats: {}", e),
        }
    });
}
