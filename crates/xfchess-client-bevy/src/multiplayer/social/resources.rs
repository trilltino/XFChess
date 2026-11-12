use bevy::prelude::*;
use crossbeam_channel::Receiver;
use std::time::Instant;
#[derive(Resource, Default)]
pub struct OnlinePlayersState {
    pub count: usize,
    pub last_sync: Option<Instant>,
    pub fetch_rx: Option<Receiver<usize>>,
}

pub(super) const OPPONENT_PRESENCE_TIMEOUT_SECS: i64 = 6;

#[derive(Resource, Default)]
pub struct OpponentLivenessState {
    pub(super) opponent_node_id: Option<String>,
    pub(super) opponent_last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) since_last_poll: f32,
    pub(super) fetch_rx: Option<Receiver<Option<chrono::DateTime<chrono::Utc>>>>,
}

impl OpponentLivenessState {
    pub fn is_opponent_stale(&self) -> bool {
        self.opponent_last_seen
            .map(|t| {
                chrono::Utc::now() - t > chrono::Duration::seconds(OPPONENT_PRESENCE_TIMEOUT_SECS)
            })
            .unwrap_or(false)
    }
}

#[derive(Resource, Default)]
pub struct BackendRegion {
    pub tag: String,
    pub label: String,
    pub latency_ms: Option<u32>,
}

#[derive(Resource, Default)]
pub struct LobbyFilterConfig {
    pub time_min: Option<u32>,
    pub time_max: Option<u32>,
    pub stake_min: Option<f64>,
    pub stake_max: Option<f64>,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    pub sort: LobbySort,
    pub dirty: bool, // set to true when filters change to force re-fetch
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum LobbySort {
    #[default]
    Newest,
    EloAsc,
    EloDesc,
    StakeAsc,
    StakeDesc,
    TimeAsc,
}

impl LobbySort {
    pub fn as_str(&self) -> &'static str {
        match self {
            LobbySort::Newest => "newest",
            LobbySort::EloAsc => "elo_asc",
            LobbySort::EloDesc => "elo_desc",
            LobbySort::StakeAsc => "stake_asc",
            LobbySort::StakeDesc => "stake_desc",
            LobbySort::TimeAsc => "time_asc",
        }
    }
}

#[derive(Resource, Default)]
pub struct LobbyFetchState {
    pub status: LobbyLoadStatus,
    pub games: Vec<crate::multiplayer::vps_client::P2PGameListing>,
    pub last_fetch: Option<Instant>,
    pub rx: Option<Receiver<Result<Vec<crate::multiplayer::vps_client::P2PGameListing>, String>>>,
}

#[derive(Default, PartialEq, Clone)]
pub enum LobbyLoadStatus {
    #[default]
    Idle,
    Fetching,
    Done,
    Error(String),
}
