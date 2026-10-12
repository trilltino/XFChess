//! A Swiss event's state as stored on its tournament record, and the questions the
//! rest of the backend asks of it: who plays whom now, which games exist, how far
//! the event has got.
//!
//! Rounds and boards are 1-based here, as the pairing engine numbers them. Every
//! method that reports a round to callers outside the Swiss module returns it
//! 0-based, like bracket rounds.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use swiss_pairing::{MatchResult, Pairing, SwissRound};

use crate::signing::storage::tournament::{
    AssignedGame, GameOutcome, MatchAssignment, MatchStatus, TournamentStore,
};

impl From<GameOutcome> for MatchResult {
    fn from(outcome: GameOutcome) -> Self {
        match outcome {
            GameOutcome::WhiteWin => Self::WhiteWin,
            GameOutcome::BlackWin => Self::BlackWin,
            GameOutcome::Draw => Self::Draw,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SwissStorageData {
    pub current_round: u8,
    pub total_rounds: u8,
    pub rounds: Vec<SwissRound>,
    pub results: Vec<(u8, u16, MatchResult)>,
    pub standings: Vec<swiss_pairing::StandingsEntry>,
    #[serde(default)]
    pub round_deadline_at: Option<i64>,
    #[serde(default)]
    pub absent_players: Vec<String>,
    #[serde(default)]
    pub withdrawn_players: Vec<String>,
    #[serde(default)]
    pub forbidden_pairs: Vec<(String, String)>,
    #[serde(default)]
    pub manual_pairings_next_round: Vec<swiss_pairing::ManualPairing>,
    /// `(round, board)` results already written on-chain via `record_swiss_result`
    /// (backend 1-based numbering). Lets the chain reconciler resume after a restart.
    #[serde(default)]
    pub chain_boards_recorded: Vec<(u8, u16)>,
    /// Set once `complete_swiss_tournament` has landed on-chain.
    #[serde(default)]
    pub chain_completed: bool,
}

impl SwissStorageData {
    /// An event of `total_rounds` rounds with nothing paired yet.
    pub fn new(total_rounds: u8) -> Self {
        Self {
            total_rounds,
            ..Self::default()
        }
    }

    pub fn has_result(&self, round: u8, board: u16) -> bool {
        self.results
            .iter()
            .any(|(r, b, _)| *r == round && *b == board)
    }

    pub fn result(&self, round: u8, board: u16) -> Option<MatchResult> {
        self.results
            .iter()
            .find(|(r, b, _)| *r == round && *b == board)
            .map(|(_, _, result)| *result)
    }

    /// The board's winner; `None` while it is open and when it was drawn.
    pub fn board_winner<'p>(&self, round: u8, pairing: &'p Pairing) -> Option<&'p str> {
        match self.result(round, pairing.board)? {
            MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin => Some(&pairing.white),
            MatchResult::BlackWin | MatchResult::ForfeitBlackWin => Some(&pairing.black),
            _ => None,
        }
    }

    pub fn round(&self, round: u8) -> Option<&SwissRound> {
        self.rounds.iter().find(|r| r.round == round)
    }

    /// Every board of `round` has a result (a round with no boards is complete).
    pub fn round_complete(&self, round: u8) -> bool {
        self.round(round)
            .is_some_and(|r| r.pairings.iter().all(|p| self.has_result(round, p.board)))
    }

    pub fn is_withdrawn(&self, player: &str) -> bool {
        self.withdrawn_players.iter().any(|p| p == player)
    }

    /// 0-based round in play; `None` before round 1 is paired.
    pub fn current_round_index(&self) -> Option<u8> {
        self.current_round.checked_sub(1)
    }

    /// Boards without a result in the 0-based `round`.
    pub fn open_boards_in(&self, round: u8) -> usize {
        let swiss_round = round + 1;
        self.round(swiss_round).map_or(0, |r| {
            r.pairings
                .iter()
                .filter(|p| !self.has_result(swiss_round, p.board))
                .count()
        })
    }

    /// The player's assignment in the latest round: their bye, or their board while
    /// it has no result yet. `None` once their game is decided.
    pub fn assignment_for(
        &self,
        tournament_id: u64,
        player: &str,
        node_ids: &HashMap<String, String>,
    ) -> Option<MatchAssignment> {
        let round = self.rounds.last()?;
        let round_index = round.round.checked_sub(1);

        if round.byes.iter().any(|bye| bye == player) {
            return Some(MatchAssignment {
                match_index: 0,
                round: round_index,
                board: None,
                game_id: None,
                opponent_pubkey: String::new(),
                opponent_node_id: None,
                your_color: "bye".to_string(),
                status: MatchStatus::Completed,
                is_bye: true,
            });
        }

        let (index, pairing) = round
            .pairings
            .iter()
            .enumerate()
            .find(|(_, p)| p.white == player || p.black == player)?;
        if self.has_result(round.round, pairing.board) {
            return None;
        }
        let is_white = pairing.white == player;
        let opponent = if is_white {
            &pairing.black
        } else {
            &pairing.white
        };

        Some(MatchAssignment {
            match_index: index as u16,
            round: round_index,
            board: Some(pairing.board),
            game_id: TournamentStore::swiss_game_id(tournament_id, round.round, pairing.board),
            opponent_node_id: node_ids.get(opponent).cloned(),
            opponent_pubkey: opponent.clone(),
            your_color: if is_white { "white" } else { "black" }.to_string(),
            status: MatchStatus::Active,
            is_bye: false,
        })
    }

    /// The latest round's boards that still need a game played.
    pub fn open_games(&self, tournament_id: u64) -> Vec<AssignedGame> {
        let Some(round) = self.rounds.last() else {
            return Vec::new();
        };
        round
            .pairings
            .iter()
            .filter(|p| !self.has_result(round.round, p.board))
            .filter_map(|p| {
                Some(AssignedGame {
                    tournament_id,
                    game_id: TournamentStore::swiss_game_id(tournament_id, round.round, p.board)?,
                    white: p.white.clone(),
                    black: p.black.clone(),
                })
            })
            .collect()
    }

    /// Every board game this event has paired, played or not.
    pub fn game_ids(&self, tournament_id: u64) -> Vec<u64> {
        self.rounds
            .iter()
            .flat_map(|round| {
                round.pairings.iter().filter_map(move |p| {
                    TournamentStore::swiss_game_id(tournament_id, round.round, p.board)
                })
            })
            .collect()
    }

    /// The 0-based round `game_id` was played in, if it is one of this event's boards.
    pub fn round_of_game(&self, tournament_id: u64, game_id: u64) -> Option<u8> {
        let (round, board) = TournamentStore::swiss_board_for_game_id(tournament_id, game_id)?;
        self.round(round)?
            .pairings
            .iter()
            .any(|p| p.board == board)
            .then(|| round - 1)
    }

    /// Every board in the bracket view's match shape: 0-based `round` (like bracket
    /// rounds), `board`, the game id, and a winner once decided (a draw completes the
    /// board with no winner).
    pub fn bracket_matches(&self, tournament_id: u64) -> serde_json::Value {
        let matches: Vec<serde_json::Value> = self
            .rounds
            .iter()
            .flat_map(|round| round.pairings.iter().map(move |p| (round.round, p)))
            .enumerate()
            .map(|(index, (round, p))| {
                serde_json::json!({
                    "match_index": index,
                    "round": round.saturating_sub(1),
                    "board": p.board,
                    "player_white": p.white,
                    "player_black": p.black,
                    "winner": self.board_winner(round, p),
                    "game_id": TournamentStore::swiss_game_id(tournament_id, round, p.board),
                    "status": if self.has_result(round, p.board) { "Completed" } else { "Active" },
                    "next_match_for_winner": null,
                })
            })
            .collect();
        serde_json::Value::Array(matches)
    }
}
