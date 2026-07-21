//! Live SOL/USD rate fetching for wager tier pricing.
//!
//! This module keeps a cached SOL/USD quote in a Bevy resource and refreshes
//! it from the VPS-backed backend endpoint every 60 seconds so the main menu
//! can display live SOL values for the fixed USD wager tiers. USD is the
//! primary currency shown throughout the game client and admin panel — SOL
//! remains the on-chain settlement unit and is what the wallet-ui signing
//! popup shows, but every in-game amount an admin or player types/reads
//! should be in USD.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::multiplayer::vps_client;

/// Snapshot of the latest SOL/USD exchange rate.
#[derive(Debug, Clone)]
pub struct SolUsdRateSnapshot {
    /// SOL purchasable for 1 USD.
    pub sol_per_usd: f64,
    /// USD per 1 SOL.
    pub usd_per_sol: f64,
    /// Unix timestamp when the rate was fetched.
    pub fetched_at: i64,
}

#[derive(Debug)]
enum SolUsdRateMessage {
    Success(SolUsdRateSnapshot),
    Error(String),
}

/// Cached SOL/USD exchange-rate resource used by the wager-tier UI.
#[derive(Resource)]
pub struct SolUsdRate {
    /// Latest successful exchange-rate snapshot, if one has been fetched.
    pub current: Option<SolUsdRateSnapshot>,
    /// Last time a refresh request was dispatched.
    pub last_refresh: Option<Instant>,
    /// Refresh cadence for the backend fetch loop.
    pub refresh_interval: Duration,
    /// Whether a background refresh request is currently in flight.
    pub is_refreshing: bool,
    /// Most recent fetch error, if any.
    pub last_error: Option<String>,
    response_tx: Sender<SolUsdRateMessage>,
    response_rx: Receiver<SolUsdRateMessage>,
}

impl Default for SolUsdRate {
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

impl SolUsdRate {
    /// Convert a USD wager into SOL using the latest cached rate.
    pub fn sol_for_usd(&self, usd: f64) -> Option<f64> {
        self.current.as_ref().map(|rate| usd * rate.sol_per_usd)
    }

    /// Convert a SOL amount into USD using the latest cached rate.
    pub fn usd_for_sol(&self, sol: f64) -> Option<f64> {
        self.current.as_ref().map(|rate| sol * rate.usd_per_sol)
    }

    /// Return the latest cached SOL/USD snapshot, if available.
    pub fn snapshot(&self) -> Option<&SolUsdRateSnapshot> {
        self.current.as_ref()
    }
}

/// Plugin that keeps the SOL/USD rate cache warm.
pub struct SolUsdRatePlugin;

impl Plugin for SolUsdRatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolUsdRate>()
            .add_systems(Startup, kick_off_sol_usd_refresh)
            .add_systems(Update, poll_sol_usd_refresh);
    }
}

/// Immediately start fetching the live SOL/USD rate when the app boots.
fn kick_off_sol_usd_refresh(mut rate: ResMut<SolUsdRate>) {
    dispatch_refresh(&mut rate);
}

/// Poll for completed refreshes and schedule the next backend fetch.
fn poll_sol_usd_refresh(mut rate: ResMut<SolUsdRate>) {
    while let Ok(message) = rate.response_rx.try_recv() {
        match message {
            SolUsdRateMessage::Success(snapshot) => {
                info!(
                    "[SOL_USD_RATE] Refreshed SOL/USD quote: 1 SOL = ${:.2}",
                    snapshot.usd_per_sol
                );
                rate.current = Some(snapshot);
                rate.last_error = None;
                rate.is_refreshing = false;
            }
            SolUsdRateMessage::Error(err) => {
                warn!("[SOL_USD_RATE] Refresh failed: {}", err);
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
fn dispatch_refresh(rate: &mut SolUsdRate) {
    rate.is_refreshing = true;
    rate.last_refresh = Some(Instant::now());

    let response_tx = rate.response_tx.clone();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            match vps_client::fetch_sol_usd_rate() {
                Ok(payload) => {
                    let _ = response_tx.send(SolUsdRateMessage::Success(SolUsdRateSnapshot {
                        sol_per_usd: payload.sol_per_usd,
                        usd_per_sol: payload.usd_per_sol,
                        fetched_at: payload.fetched_at,
                    }));
                }
                Err(err) => {
                    error!("[SOL_USD_RATE] Backend fetch error: {}", err);
                    let _ = response_tx.send(SolUsdRateMessage::Error(err));
                }
            }
        })
        .detach();
}
