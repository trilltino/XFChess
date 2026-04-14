use bevy::prelude::*;
use crate::multiplayer::vps_client::TournamentSummary;
use tokio::sync::oneshot;

#[derive(Resource, Default)]
pub struct TournamentLobbyState {
    #[allow(dead_code)]
    pub refreshing: bool,
    #[allow(dead_code)]
    pub joining_id: Option<u64>,
    /// Receiver for the tournament list fetch task
    #[allow(dead_code)]
    pub fetch_rx: Option<oneshot::Receiver<Result<Vec<TournamentSummary>, String>>>,
    /// Receiver for the join tournament task
    #[allow(dead_code)]
    pub join_rx: Option<oneshot::Receiver<Result<u32, String>>>,
}
