use crate::multiplayer::network::vps::tournament::TournamentSummary;
use bevy::prelude::*;
use tokio::sync::oneshot;

#[derive(Resource, Default)]
pub struct TournamentLobbyState {
    pub refreshing: bool,
    pub joining_id: Option<u64>,
    pub fetch_rx: Option<oneshot::Receiver<Result<Vec<TournamentSummary>, String>>>,
    pub join_rx: Option<oneshot::Receiver<Result<u32, String>>>,
    pub swiss_standings: Option<Vec<SwissStanding>>,
    pub swiss_current_round: Option<u8>,
    pub swiss_total_rounds: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct SwissStanding {
    pub player: String,
    pub score: u8,
    pub buchholz: u16,
    pub sonneborn: u16,
    pub color_balance: i8,
}
