//! Tournament client for real-time Swiss tournament updates via gossip.
//!
//! This plugin integrates with the braid-iroh gossip protocol to receive
//! instant updates for pairings, results, and standings.

use bevy::prelude::*;
use braid_iroh::protocol::{SwissMessage, SwissPairing, SwissStandingsEntry};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::multiplayer::braid_network::BraidNetworkState;

/// Plugin for tournament gossip client
pub struct TournamentClientPlugin;

impl Plugin for TournamentClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TournamentClientState>()
            .add_event::<RoundStarted>()
            .add_event::<ResultRecorded>()
            .add_event::<StandingsUpdated>()
            .add_systems(Update, process_gossip_messages);
    }
}

/// State of the tournament client
#[derive(Resource, Default)]
pub struct TournamentClientState {
    /// Currently active tournament ID
    pub active_tournament: Option<u64>,
    /// Gossip message receiver
    gossip_rx: Option<mpsc::UnboundedReceiver<SwissMessage>>,
    /// Current round number
    pub current_round: u8,
    /// Current pairings for the player
    pub my_pairing: Option<PlayerPairing>,
    /// Current standings
    pub standings: Vec<StandingsEntry>,
    /// Player's current rank
    pub my_rank: Option<u16>,
    /// Player's wallet pubkey
    pub player_id: Option<String>,
}

/// Player's pairing information
#[derive(Debug, Clone)]
pub struct PlayerPairing {
    pub round: u8,
    pub board: u16,
    pub opponent: String,
    pub color: PlayerColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerColor {
    White,
    Black,
}

/// Standings entry for UI display
#[derive(Debug, Clone)]
pub struct StandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub rank: u16,
}

/// Event emitted when a new round starts
#[derive(Event)]
pub struct RoundStarted {
    pub tournament_id: u64,
    pub round: u8,
    pub my_pairing: Option<PlayerPairing>,
}

/// Event emitted when a result is recorded
#[derive(Event)]
pub struct ResultRecorded {
    pub tournament_id: u64,
    pub round: u8,
    pub board: u16,
    pub white_score: f64,
    pub black_score: f64,
}

/// Event emitted when standings are updated
#[derive(Event)]
pub struct StandingsUpdated {
    pub tournament_id: u64,
    pub standings: Vec<StandingsEntry>,
    pub my_rank: Option<u16>,
}

impl TournamentClientState {
    /// Join a tournament and subscribe to gossip updates
    pub fn join_tournament(
        &mut self,
        tournament_id: u64,
        player_id: String,
    ) {
        self.active_tournament = Some(tournament_id);
        let player_id_clone = player_id.clone();
        self.player_id = Some(player_id);
        self.current_round = 0;
        self.my_pairing = None;
        self.standings.clear();
        self.my_rank = None;

        info!("[tournament-client] Joined tournament {} as player {}", tournament_id, player_id_clone);
    }

    /// Leave the current tournament
    pub fn leave_tournament(&mut self) {
        if let Some(id) = self.active_tournament {
            info!("[tournament-client] Left tournament {}", id);
        }
        self.active_tournament = None;
        self.gossip_rx = None;
        self.current_round = 0;
        self.my_pairing = None;
        self.standings.clear();
        self.my_rank = None;
    }

    /// Set the gossip receiver channel
    pub fn set_gossip_receiver(&mut self, rx: mpsc::UnboundedReceiver<SwissMessage>) {
        self.gossip_rx = Some(rx);
    }

    /// Check if currently in a tournament
    pub fn is_in_tournament(&self) -> bool {
        self.active_tournament.is_some()
    }

    /// Get the active tournament ID
    pub fn active_tournament(&self) -> Option<u64> {
        self.active_tournament
    }
}

/// Process incoming gossip messages and emit Bevy events
fn process_gossip_messages(
    mut client_state: ResMut<TournamentClientState>,
    mut round_started_events: EventWriter<RoundStarted>,
    mut result_recorded_events: EventWriter<ResultRecorded>,
    mut standings_updated_events: EventWriter<StandingsUpdated>,
) {
    let Some(rx) = client_state.gossip_rx.as_mut() else {
        return;
    };

    // Process all pending messages without blocking
    while let Ok(message) = rx.try_recv() {
        let player_id = client_state.player_id.clone();
        let tournament_id = client_state.active_tournament;

        match message {
            SwissMessage::RoundStarted {
                tournament_id: msg_tournament_id,
                round,
                pairings,
            } => {
                info!("[tournament-client] Round {} started in tournament {}", round, msg_tournament_id);

                client_state.current_round = round;

                // Find the player's pairing
                let my_pairing = player_id.as_ref().and_then(|pid| {
                    find_player_pairing(pid, round, &pairings)
                });

                client_state.my_pairing = my_pairing.clone();

                round_started_events.send(RoundStarted {
                    tournament_id: msg_tournament_id,
                    round,
                    my_pairing,
                });
            }
            SwissMessage::ResultRecorded {
                tournament_id: msg_tournament_id,
                round,
                board,
                result,
            } => {
                info!("[tournament-client] Result recorded for tournament {} round {} board {}",
                    msg_tournament_id, round, board);

                let (white_score, black_score) = match result {
                    braid_iroh::protocol::MatchResult::Win { winner } => {
                        // Need to look up who was white/black in the pairing
                        // For now, assume 1-0 or 0-1
                        (1.0, 0.0) // Simplified - would need pairing info
                    }
                    braid_iroh::protocol::MatchResult::Draw => (0.5, 0.5),
                };

                result_recorded_events.send(ResultRecorded {
                    tournament_id: msg_tournament_id,
                    round,
                    board,
                    white_score,
                    black_score,
                });
            }
            SwissMessage::StandingsUpdated {
                tournament_id: msg_tournament_id,
                standings,
            } => {
                info!("[tournament-client] Standings updated for tournament {} ({} entries)",
                    msg_tournament_id, standings.len());

                let entries: Vec<StandingsEntry> = standings
                    .iter()
                    .map(|s| StandingsEntry {
                        player_id: s.player_id.clone(),
                        score: s.score,
                        rank: s.rank,
                    })
                    .collect();

                // Find player's rank
                let my_rank = player_id.as_ref().and_then(|pid| {
                    entries.iter().find(|e| e.player_id == *pid).map(|e| e.rank)
                });

                client_state.standings = entries.clone();
                client_state.my_rank = my_rank;

                standings_updated_events.send(StandingsUpdated {
                    tournament_id: msg_tournament_id,
                    standings: entries,
                    my_rank,
                });
            }
        }
    }
}

/// Find the pairing for a specific player
fn find_player_pairing(
    player_id: &str,
    round: u8,
    pairings: &[SwissPairing],
) -> Option<PlayerPairing> {
    for pairing in pairings {
        if pairing.white == player_id {
            return Some(PlayerPairing {
                round,
                board: pairing.board,
                opponent: pairing.black.clone(),
                color: PlayerColor::White,
            });
        }
        if pairing.black == player_id {
            return Some(PlayerPairing {
                round,
                board: pairing.board,
                opponent: pairing.white.clone(),
                color: PlayerColor::Black,
            });
        }
    }
    None
}

/// System to initialize tournament client when joining a tournament
pub fn join_tournament_system(
    mut commands: Commands,
    mut client_state: ResMut<TournamentClientState>,
    braid_state: Res<BraidNetworkState>,
) {
    // This would be triggered by a UI action or network response
    // For now, it's a placeholder for the integration point
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tournament_client_state_default() {
        let state = TournamentClientState::default();
        assert!(state.active_tournament.is_none());
        assert_eq!(state.current_round, 0);
        assert!(state.my_pairing.is_none());
        assert!(state.standings.is_empty());
    }

    #[test]
    fn test_join_tournament() {
        let mut state = TournamentClientState::default();
        state.join_tournament(123, "player1".to_string());

        assert_eq!(state.active_tournament, Some(123));
        assert_eq!(state.player_id, Some("player1".to_string()));
        assert!(state.is_in_tournament());
    }

    #[test]
    fn test_leave_tournament() {
        let mut state = TournamentClientState::default();
        state.join_tournament(123, "player1".to_string());
        state.leave_tournament();

        assert!(state.active_tournament.is_none());
        assert!(!state.is_in_tournament());
    }

    #[test]
    fn test_find_player_pairing_white() {
        let pairings = vec![
            SwissPairing {
                white: "player1".to_string(),
                black: "player2".to_string(),
                board: 1,
            },
            SwissPairing {
                white: "player3".to_string(),
                black: "player4".to_string(),
                board: 2,
            },
        ];

        let pairing = find_player_pairing("player1", 1, &pairings);
        assert!(pairing.is_some());
        let p = pairing.unwrap();
        assert_eq!(p.round, 1);
        assert_eq!(p.board, 1);
        assert_eq!(p.opponent, "player2");
        assert!(matches!(p.color, PlayerColor::White));
    }

    #[test]
    fn test_find_player_pairing_black() {
        let pairings = vec![
            SwissPairing {
                white: "player1".to_string(),
                black: "player2".to_string(),
                board: 1,
            },
        ];

        let pairing = find_player_pairing("player2", 1, &pairings);
        assert!(pairing.is_some());
        let p = pairing.unwrap();
        assert_eq!(p.opponent, "player1");
        assert!(matches!(p.color, PlayerColor::Black));
    }

    #[test]
    fn test_find_player_pairing_not_found() {
        let pairings = vec![
            SwissPairing {
                white: "player1".to_string(),
                black: "player2".to_string(),
                board: 1,
            },
        ];

        let pairing = find_player_pairing("player3", 1, &pairings);
        assert!(pairing.is_none());
    }
}
