//! Tournament events for Bevy event system.
//!
//! These events are emitted by the TournamentClient when gossip messages
//! are received, allowing UI systems to react to tournament updates.

use bevy::prelude::*;

/// Event emitted when a new round starts
#[derive(Event, Debug, Clone)]
pub struct RoundStarted {
    pub tournament_id: u64,
    pub round: u8,
    pub pairings: Vec<PairingInfo>,
    pub my_pairing: Option<MyPairing>,
}

/// Event emitted when a match result is recorded
#[derive(Event, Debug, Clone)]
pub struct ResultRecorded {
    pub tournament_id: u64,
    pub round: u8,
    pub board: u16,
    pub result: MatchResult,
}

/// Event emitted when tournament standings are updated
#[derive(Event, Debug, Clone)]
pub struct StandingsUpdated {
    pub tournament_id: u64,
    pub standings: Vec<StandingsEntry>,
    pub my_rank: Option<u16>,
}

/// Event emitted when player joins a tournament
#[derive(Event, Debug, Clone)]
pub struct TournamentJoined {
    pub tournament_id: u64,
    pub player_id: String,
    pub bootstrap_peers: Vec<String>,
    pub topic_url: String,
}

/// Event emitted when player leaves a tournament
#[derive(Event, Debug, Clone)]
pub struct TournamentLeft {
    pub tournament_id: u64,
}

/// Event emitted when tournament is completed
#[derive(Event, Debug, Clone)]
pub struct TournamentCompleted {
    pub tournament_id: u64,
    pub winner: String,
    pub final_standings: Vec<StandingsEntry>,
}

/// Pairing information for a match
#[derive(Debug, Clone)]
pub struct PairingInfo {
    pub board: u16,
    pub white: String,
    pub black: String,
}

/// Player's specific pairing information
#[derive(Debug, Clone)]
pub struct MyPairing {
    pub round: u8,
    pub board: u16,
    pub opponent: String,
    pub color: PlayerColor,
}

/// Player color assignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerColor {
    White,
    Black,
}

/// Match result
#[derive(Debug, Clone)]
pub enum MatchResult {
    Win { winner: String },
    Draw,
}

/// Standings entry
#[derive(Debug, Clone)]
pub struct StandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub rank: u16,
    pub tiebreak: Option<f64>,
}

/// Plugin that registers all tournament events
pub struct TournamentEventsPlugin;

impl Plugin for TournamentEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RoundStarted>()
            .add_event::<ResultRecorded>()
            .add_event::<StandingsUpdated>()
            .add_event::<TournamentJoined>()
            .add_event::<TournamentLeft>()
            .add_event::<TournamentCompleted>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_started_event() {
        let event = RoundStarted {
            tournament_id: 123,
            round: 1,
            pairings: vec![PairingInfo {
                board: 1,
                white: "player1".to_string(),
                black: "player2".to_string(),
            }],
            my_pairing: Some(MyPairing {
                round: 1,
                board: 1,
                opponent: "player2".to_string(),
                color: PlayerColor::White,
            }),
        };
        assert_eq!(event.tournament_id, 123);
        assert_eq!(event.round, 1);
    }

    #[test]
    fn test_result_recorded_event() {
        let event = ResultRecorded {
            tournament_id: 123,
            round: 1,
            board: 1,
            result: MatchResult::Win {
                winner: "player1".to_string(),
            },
        };
        assert_eq!(event.tournament_id, 123);
        assert_eq!(event.board, 1);
    }

    #[test]
    fn test_standings_updated_event() {
        let event = StandingsUpdated {
            tournament_id: 123,
            standings: vec![
                StandingsEntry {
                    player_id: "player1".to_string(),
                    score: 1.0,
                    rank: 1,
                    tiebreak: None,
                },
            ],
            my_rank: Some(1),
        };
        assert_eq!(event.tournament_id, 123);
        assert_eq!(event.my_rank, Some(1));
    }

    #[test]
    fn test_tournament_joined_event() {
        let event = TournamentJoined {
            tournament_id: 123,
            player_id: "player1".to_string(),
            bootstrap_peers: vec!["peer1".to_string()],
            topic_url: "/swiss/123".to_string(),
        };
        assert_eq!(event.topic_url, "/swiss/123");
    }

    #[test]
    fn test_player_color_equality() {
        assert_eq!(PlayerColor::White, PlayerColor::White);
        assert_ne!(PlayerColor::White, PlayerColor::Black);
    }
}
