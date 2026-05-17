use bevy::prelude::*;
use backend_types::tournament::TournamentSummary;
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
    /// Swiss tournament standings (if in a Swiss tournament)
    pub swiss_standings: Option<Vec<SwissStanding>>,
    /// Current Swiss round (if in a Swiss tournament)
    pub swiss_current_round: Option<u8>,
    /// Total Swiss rounds (if in a Swiss tournament)
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
