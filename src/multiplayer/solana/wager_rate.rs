//! Live SOL/GBP rate fetching for wager tier pricing.
//!
//! This module keeps a cached SOL/GBP quote in a Bevy resource and refreshes
//! it from the VPS-backed backend endpoint every 60 seconds so the main menu
//! can display live SOL values for the fixed GBP wager tiers.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::multiplayer::vps_client;

/// Snapshot of the latest SOL/GBP exchange rate.
#[derive(Debug, Clone)]
pub struct SolGbpRateSnapshot {
    /// SOL purchasable for 1 GBP.
    pub sol_per_gbp: f64,
    /// GBP per 1 SOL.
    pub gbp_per_sol: f64,
    /// Unix timestamp when the rate was fetched.
    pub fetched_at: i64,
}

#[derive(Debug)]
enum SolGbpRateMessage {
    Success(SolGbpRateSnapshot),
    Error(String),
}

/// Cached SOL/GBP exchange-rate resource used by the wager-tier UI.
#[derive(Resource)]
pub struct SolGbpRate {
    /// Latest successful exchange-rate snapshot, if one has been fetched.
    pub current: Option<SolGbpRateSnapshot>,
    /// Last time a refresh request was dispatched.
    pub last_refresh: Option<Instant>,
    /// Refresh cadence for the backend fetch loop.
    pub refresh_interval: Duration,
    /// Whether a background refresh request is currently in flight.
    pub is_refreshing: bool,
    /// Most recent fetch error, if any.
    pub last_error: Option<String>,
    response_tx: Sender<SolGbpRateMessage>,
    response_rx: Receiver<SolGbpRateMessage>,
}

impl Default for SolGbpRate {
    fn default() -> Self {
        let (response_tx, response_rx) = unbounded();
        Self {
            current: None,
            last_refresh: None,
            refresh_interval: Duration::from_secs(60),
            is_refreshing: false,
            last_error: None,
            response_tx,
            response_rx,
        }
    }
}

impl SolGbpRate {
    /// Convert a GBP wager into SOL using the latest cached rate.
    pub fn sol_for_gbp(&self, gbp: f64) -> Option<f64> {
        self.current.as_ref().map(|rate| gbp * rate.sol_per_gbp)
    }

    /// Return the latest cached SOL/GBP snapshot, if available.
    pub fn snapshot(&self) -> Option<&SolGbpRateSnapshot> {
        self.current.as_ref()
    }
}

/// Plugin that keeps the SOL/GBP rate cache warm.
pub struct SolGbpRatePlugin;

impl Plugin for SolGbpRatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolGbpRate>()
            .add_systems(Startup, kick_off_sol_gbp_refresh)
            .add_systems(Update, poll_sol_gbp_refresh);
    }
}

/// Immediately start fetching the live SOL/GBP rate when the app boots.
fn kick_off_sol_gbp_refresh(mut rate: ResMut<SolGbpRate>) {
    dispatch_refresh(&mut rate);
}

/// Poll for completed refreshes and schedule the next backend fetch.
fn poll_sol_gbp_refresh(mut rate: ResMut<SolGbpRate>) {
    while let Ok(message) = rate.response_rx.try_recv() {
        match message {
            SolGbpRateMessage::Success(snapshot) => {
                info!(
                    "[SOL_GBP_RATE] Refreshed SOL/GBP quote: 1 SOL = £{:.4}",
                    snapshot.gbp_per_sol
                );
                rate.current = Some(snapshot);
                rate.last_error = None;
                rate.is_refreshing = false;
            }
            SolGbpRateMessage::Error(err) => {
                warn!("[SOL_GBP_RATE] Refresh failed: {}", err);
                rate.last_error = Some(err);
                rate.is_refreshing = false;
            }
        }
    }

    let stale = rate
        .last_refresh
        .map(|instant| instant.elapsed() >= rate.refresh_interval)
        .unwrap_or(true);

    if stale && !rate.is_refreshing {
        dispatch_refresh(&mut rate);
    }
}

/// Dispatch a background refresh request against the backend rate endpoint.
fn dispatch_refresh(rate: &mut SolGbpRate) {
    rate.is_refreshing = true;
    rate.last_refresh = Some(Instant::now());

    let response_tx = rate.response_tx.clone();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            match vps_client::fetch_sol_gbp_rate() {
                Ok(payload) => {
                    let _ = response_tx.send(SolGbpRateMessage::Success(SolGbpRateSnapshot {
                        sol_per_gbp: payload.sol_per_gbp,
                        gbp_per_sol: payload.gbp_per_sol,
                        fetched_at: payload.fetched_at,
                    }));
                }
                Err(err) => {
                    error!("[SOL_GBP_RATE] Backend fetch error: {}", err);
                    let _ = response_tx.send(SolGbpRateMessage::Error(err));
                }
            }
        })
        .detach();
}
