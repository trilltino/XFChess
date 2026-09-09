use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::multiplayer::vps_client;

#[derive(Debug, Clone)]
pub struct SolUsdRateSnapshot {
    pub sol_per_usd: f64,
    pub usd_per_sol: f64,
    pub fetched_at: i64,
}

#[derive(Debug)]
enum SolUsdRateMessage {
    Success(SolUsdRateSnapshot),
    Error(String),
}

#[derive(Resource)]
pub struct SolUsdRate {
    pub current: Option<SolUsdRateSnapshot>,
    pub last_refresh: Option<Instant>,
    pub refresh_interval: Duration,
    pub is_refreshing: bool,
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
    pub fn sol_for_usd(&self, usd: f64) -> Option<f64> {
        self.current.as_ref().map(|rate| usd * rate.sol_per_usd)
    }

    pub fn usd_for_sol(&self, sol: f64) -> Option<f64> {
        self.current.as_ref().map(|rate| sol * rate.usd_per_sol)
    }

    pub fn snapshot(&self) -> Option<&SolUsdRateSnapshot> {
        self.current.as_ref()
    }
}

pub struct SolUsdRatePlugin;

impl Plugin for SolUsdRatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolUsdRate>()
            .add_systems(Startup, kick_off_sol_usd_refresh)
            .add_systems(Update, poll_sol_usd_refresh);
    }
}

fn kick_off_sol_usd_refresh(mut rate: ResMut<SolUsdRate>) {
    dispatch_refresh(&mut rate);
}

fn poll_sol_usd_refresh(mut rate: ResMut<SolUsdRate>) {
    while let Ok(message) = rate.response_rx.try_recv() {
        match message {
            SolUsdRateMessage::Success(snapshot) => {
                // No per-refresh log here on purpose — this fires every 60s
                // for the lifetime of the app and added nothing but noise to
                // every log dump used to debug session/networking issues
                // this session. Failures below still log; see
                // `monitoring::log_game_health_snapshot` for the
                // periodic-status replacement.
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
