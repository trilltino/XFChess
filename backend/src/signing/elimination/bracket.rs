//! A single-elimination bracket and its rules.
//!
//! The whole bracket is drawn when the event starts: round one seeds the best player
//! against the worst, and every later match starts with empty seats that winners
//! move into. A drawn game is replayed with colours reversed; once
//! [`MAX_REPLAYS`] replays have also been drawn, the higher seed goes through.
//!
//! Rounds and match indices are 0-based, as on-chain.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::signing::solana::bracket_position;
use crate::signing::storage::tournament::{
    AssignedGame, GameOutcome, LastMatchResult, MatchAssignment, MatchStatus, ResultSource,
    TournamentStore,
};

/// Replays of a drawn match before the higher seed advances instead.
pub const MAX_REPLAYS: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentMatch {
    pub match_index: u16,
    pub round: u8,
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub winner: Option<String>,
    /// The game currently scheduled for this match (the latest replay, if any).
    pub game_id: Option<u64>,
    pub status: MatchStatus,
    pub result_source: Option<ResultSource>,
    pub next_match_for_winner: Option<u16>,
    pub next_match_slot: u8,
    /// Games of this match that ended drawn.
    #[serde(default)]
    pub drawn_games: u8,
    /// The drawn games a replay, colours reversed, replaced.
    #[serde(default)]
    pub drawn_game_ids: Vec<u64>,
    /// The result, and the winner's move into the next match, are on-chain.
    #[serde(default)]
    pub chain_synced: bool,
}

impl TournamentMatch {
    pub fn is_decided(&self) -> bool {
        self.status == MatchStatus::Completed
    }

    fn seats(&self) -> Option<(&str, &str)> {
        Some((self.player_white.as_deref()?, self.player_black.as_deref()?))
    }

    fn has_player(&self, player: &str) -> bool {
        self.player_white.as_deref() == Some(player) || self.player_black.as_deref() == Some(player)
    }

    /// The decided match's other player.
    pub fn loser(&self) -> Option<&str> {
        let winner = self.winner.as_deref()?;
        let (white, black) = self.seats()?;
        Some(if winner == white { black } else { white })
    }

    /// Every game this match has scheduled, drawn ones first.
    pub fn game_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.drawn_game_ids.iter().copied().chain(self.game_id)
    }
}

/// Where a decided match sits, which says what its result means for the placings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Final,
    /// The loser finishes in `loser_place` (the second semifinal's loser is third).
    SemiFinal {
        loser_place: u8,
    },
    Earlier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub match_index: u16,
    pub winner: String,
    pub loser: String,
    pub stage: Stage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameApplied {
    Decided(Decision),
    /// The game was drawn and the match continues with a new game, colours reversed.
    Replay {
        match_index: u16,
        game_id: Option<u64>,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EliminationError {
    #[error("tournament not found")]
    TournamentNotFound,
    #[error("tournament is not single elimination")]
    NotElimination,
    #[error("tournament is not active")]
    NotActive,
    #[error("match {0} does not exist")]
    NoSuchMatch(u16),
    #[error("game {0} is not an undecided match of this bracket")]
    UnknownGame(u64),
    #[error("match {0} is already decided")]
    AlreadyDecided(u16),
    #[error("match {0} does not have both players yet")]
    SeatsOpen(u16),
    #[error("{0} is not one of match {1}'s players")]
    NotInMatch(String, u16),
    #[error("game {game_id} was played by {white} / {black}, not by the match's players")]
    PlayersMismatch {
        game_id: u64,
        white: String,
        black: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EliminationState {
    pub matches: Vec<TournamentMatch>,
}

impl EliminationState {
    /// The bracket for `seeds`, best player first. Round one's games are scheduled
    /// straight away; later matches wait for their winners. Needs a power-of-two
    /// field of at least two (creation enforces it).
    pub fn draw(tournament_id: u64, seeds: &[String]) -> Self {
        let n = seeds.len();
        if n < 2 {
            return Self::default();
        }
        let first_round = n / 2;
        let matches = (0..n - 1)
            .map(|i| {
                let (round, next, slot) = bracket_position(n as u16, i as u16);
                let (white, black) = if i < first_round {
                    (Some(seeds[i].clone()), Some(seeds[n - 1 - i].clone()))
                } else {
                    (None, None)
                };
                TournamentMatch {
                    match_index: i as u16,
                    round,
                    player_white: white,
                    player_black: black,
                    winner: None,
                    game_id: None,
                    status: MatchStatus::Pending,
                    result_source: None,
                    next_match_for_winner: next,
                    next_match_slot: slot,
                    drawn_games: 0,
                    drawn_game_ids: Vec::new(),
                    chain_synced: false,
                }
            })
            .collect();
        let mut bracket = Self { matches };
        for index in 0..bracket.matches.len() {
            bracket.schedule_if_seated(tournament_id, index);
        }
        bracket
    }

    pub fn is_drawn(&self) -> bool {
        !self.matches.is_empty()
    }

    fn get_mut(&mut self, match_index: u16) -> Result<&mut TournamentMatch, EliminationError> {
        self.matches
            .get_mut(match_index as usize)
            .filter(|m| m.match_index == match_index)
            .ok_or(EliminationError::NoSuchMatch(match_index))
    }

    /// Gives a match whose seats are both filled its game.
    fn schedule_if_seated(&mut self, tournament_id: u64, index: usize) {
        let Some(m) = self.matches.get_mut(index) else {
            return;
        };
        if m.status != MatchStatus::Pending || m.game_id.is_some() || m.seats().is_none() {
            return;
        }
        m.game_id = TournamentStore::bracket_game_id(tournament_id, m.match_index, m.drawn_games);
        if m.game_id.is_some() {
            m.status = MatchStatus::Active;
        }
    }

    pub fn stage_of(&self, match_index: u16) -> Stage {
        let Some(final_index) = self.matches.last().map(|m| m.match_index) else {
            return Stage::Earlier;
        };
        let has_semifinals = self.matches.len() >= 3;
        if match_index == final_index {
            Stage::Final
        } else if has_semifinals && match_index + 1 == final_index {
            Stage::SemiFinal { loser_place: 3 }
        } else if has_semifinals && match_index + 2 == final_index {
            Stage::SemiFinal { loser_place: 4 }
        } else {
            Stage::Earlier
        }
    }

    /// The lowest round with an undecided match.
    pub fn current_round(&self) -> Option<u8> {
        self.matches
            .iter()
            .filter(|m| !m.is_decided())
            .map(|m| m.round)
            .min()
    }

    pub fn total_rounds(&self) -> u8 {
        self.matches
            .iter()
            .map(|m| m.round)
            .max()
            .map_or(0, |round| round + 1)
    }

    pub fn open_matches_in(&self, round: u8) -> usize {
        self.matches
            .iter()
            .filter(|m| m.round == round && !m.is_decided())
            .count()
    }

    /// The player's undecided match, once both of its players are known.
    pub fn assignment_for(
        &self,
        player: &str,
        node_ids: &HashMap<String, String>,
    ) -> Option<MatchAssignment> {
        let m = self
            .matches
            .iter()
            .find(|m| !m.is_decided() && m.has_player(player))?;
        let (white, black) = m.seats()?;
        let is_white = white == player;
        let opponent = if is_white { black } else { white };
        Some(MatchAssignment {
            match_index: m.match_index,
            round: Some(m.round),
            board: None,
            game_id: m.game_id,
            opponent_pubkey: opponent.to_string(),
            opponent_node_id: node_ids.get(opponent).cloned(),
            your_color: if is_white { "white" } else { "black" }.to_string(),
            status: m.status.clone(),
            is_bye: false,
        })
    }

    pub fn last_result_for(&self, player: &str) -> Option<LastMatchResult> {
        let m = self
            .matches
            .iter()
            .filter(|m| m.is_decided() && m.has_player(player))
            .max_by_key(|m| m.round)?;
        let (white, black) = m.seats()?;
        Some(LastMatchResult {
            round: m.round,
            won: m.winner.as_deref() == Some(player),
            opponent: if white == player { black } else { white }.to_string(),
        })
    }

    pub fn is_knocked_out(&self, player: &str) -> bool {
        self.matches
            .iter()
            .any(|m| m.is_decided() && m.has_player(player) && m.winner.as_deref() != Some(player))
    }

    /// Undecided matches whose game can be played now.
    pub fn open_games(&self, tournament_id: u64) -> Vec<AssignedGame> {
        self.matches
            .iter()
            .filter(|m| !m.is_decided())
            .filter_map(|m| {
                let (white, black) = m.seats()?;
                Some(AssignedGame {
                    tournament_id,
                    game_id: m.game_id?,
                    white: white.to_string(),
                    black: black.to_string(),
                })
            })
            .collect()
    }

    /// Every game the bracket has scheduled, replays included.
    pub fn game_ids(&self) -> Vec<u64> {
        self.matches
            .iter()
            .flat_map(TournamentMatch::game_ids)
            .collect()
    }

    pub fn match_for_game(&self, game_id: u64) -> Option<&TournamentMatch> {
        self.matches
            .iter()
            .find(|m| m.game_ids().any(|id| id == game_id))
    }

    /// Decided matches whose result is not on-chain yet, in bracket order.
    pub fn unsynced(&self) -> impl Iterator<Item = &TournamentMatch> {
        self.matches
            .iter()
            .filter(|m| m.is_decided() && !m.chain_synced)
    }

    pub fn mark_synced(&mut self, match_index: u16) {
        if let Ok(m) = self.get_mut(match_index) {
            m.chain_synced = true;
        }
    }

    /// Records a finished game. A win decides its match; a draw schedules a replay
    /// with colours reversed until [`MAX_REPLAYS`] replays have been drawn, then the
    /// higher seed (earlier in `seeds`) goes through.
    pub fn apply_game(
        &mut self,
        tournament_id: u64,
        game_id: u64,
        white: &str,
        black: &str,
        outcome: GameOutcome,
        seeds: &[String],
    ) -> Result<GameApplied, EliminationError> {
        let m = self
            .matches
            .iter_mut()
            .find(|m| m.game_id == Some(game_id) && !m.is_decided())
            .ok_or(EliminationError::UnknownGame(game_id))?;
        let match_index = m.match_index;
        if m.seats() != Some((white, black)) {
            return Err(EliminationError::PlayersMismatch {
                game_id,
                white: white.to_string(),
                black: black.to_string(),
            });
        }

        match outcome {
            GameOutcome::WhiteWin => self
                .decide(
                    tournament_id,
                    match_index,
                    white,
                    black,
                    ResultSource::OnChain,
                )
                .map(GameApplied::Decided),
            GameOutcome::BlackWin => self
                .decide(
                    tournament_id,
                    match_index,
                    black,
                    white,
                    ResultSource::OnChain,
                )
                .map(GameApplied::Decided),
            GameOutcome::Draw if m.drawn_games >= MAX_REPLAYS => {
                m.drawn_games += 1;
                let (winner, loser) = if seed_rank(seeds, white) <= seed_rank(seeds, black) {
                    (white, black)
                } else {
                    (black, white)
                };
                self.decide(
                    tournament_id,
                    match_index,
                    winner,
                    loser,
                    ResultSource::SeedTiebreak,
                )
                .map(GameApplied::Decided)
            }
            GameOutcome::Draw => {
                m.drawn_games += 1;
                m.drawn_game_ids.push(game_id);
                std::mem::swap(&mut m.player_white, &mut m.player_black);
                m.game_id = None;
                m.status = MatchStatus::Pending;
                self.schedule_if_seated(tournament_id, match_index as usize);
                Ok(GameApplied::Replay {
                    match_index,
                    game_id: self.matches[match_index as usize].game_id,
                })
            }
        }
    }

    /// Decides a match outright (a won game, a forfeit, an admin result) and moves
    /// the winner into their next match.
    pub fn decide(
        &mut self,
        tournament_id: u64,
        match_index: u16,
        winner: &str,
        loser: &str,
        source: ResultSource,
    ) -> Result<Decision, EliminationError> {
        let stage = self.stage_of(match_index);
        let m = self.get_mut(match_index)?;
        if m.is_decided() {
            return Err(EliminationError::AlreadyDecided(match_index));
        }
        let (white, black) = m.seats().ok_or(EliminationError::SeatsOpen(match_index))?;
        if !((winner == white && loser == black) || (winner == black && loser == white)) {
            return Err(EliminationError::NotInMatch(
                winner.to_string(),
                match_index,
            ));
        }
        m.winner = Some(winner.to_string());
        m.status = MatchStatus::Completed;
        m.result_source = Some(source);
        m.chain_synced = false;
        let next = m
            .next_match_for_winner
            .map(|next| (next, m.next_match_slot));

        if let Some((next, slot)) = next {
            let seat = self.get_mut(next)?;
            if slot == 0 {
                seat.player_white = Some(winner.to_string());
            } else {
                seat.player_black = Some(winner.to_string());
            }
            self.schedule_if_seated(tournament_id, next as usize);
        }

        Ok(Decision {
            match_index,
            winner: winner.to_string(),
            loser: loser.to_string(),
            stage,
        })
    }

    /// Points `match_index` at an externally created game (admin override).
    pub fn set_game_id(&mut self, match_index: u16, game_id: u64) -> Result<(), EliminationError> {
        let m = self.get_mut(match_index)?;
        if m.is_decided() {
            return Err(EliminationError::AlreadyDecided(match_index));
        }
        m.game_id = Some(game_id);
        m.status = MatchStatus::Active;
        Ok(())
    }
}

/// 0 is the top seed; a player missing from `seeds` ranks last.
fn seed_rank(seeds: &[String], player: &str) -> usize {
    seeds
        .iter()
        .position(|seed| seed == player)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    const ID: u64 = 7;

    fn seeds(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("P{i}")).collect()
    }

    fn game(bracket: &EliminationState, index: usize) -> anyhow::Result<u64> {
        bracket.matches[index].game_id.context("scheduled")
    }

    #[test]
    fn two_players_play_a_single_final() {
        let b = EliminationState::draw(ID, &seeds(2));
        assert_eq!(b.matches.len(), 1);
        let m = &b.matches[0];
        assert_eq!(
            (m.player_white.as_deref(), m.player_black.as_deref()),
            (Some("P0"), Some("P1"))
        );
        assert_eq!(m.next_match_for_winner, None);
        assert_eq!(m.status, MatchStatus::Active);
        assert_eq!(b.stage_of(0), Stage::Final);
    }

    #[test]
    fn four_players_seed_best_against_worst_and_feed_the_final() {
        let b = EliminationState::draw(ID, &seeds(4));
        assert_eq!(b.matches.len(), 3);
        let (m0, m1, m2) = (&b.matches[0], &b.matches[1], &b.matches[2]);
        assert_eq!(
            (m0.player_white.as_deref(), m0.player_black.as_deref()),
            (Some("P0"), Some("P3"))
        );
        assert_eq!(
            (m1.player_white.as_deref(), m1.player_black.as_deref()),
            (Some("P1"), Some("P2"))
        );
        assert_eq!((m0.next_match_for_winner, m0.next_match_slot), (Some(2), 0));
        assert_eq!((m1.next_match_for_winner, m1.next_match_slot), (Some(2), 1));
        assert!(m2.player_white.is_none() && m2.game_id.is_none());
        assert_eq!(b.stage_of(0), Stage::SemiFinal { loser_place: 4 });
        assert_eq!(b.stage_of(1), Stage::SemiFinal { loser_place: 3 });
    }

    #[test]
    fn eight_player_bracket_points_forward_and_stays_in_bounds() {
        let b = EliminationState::draw(ID, &seeds(8));
        assert_eq!(b.matches.len(), 7);
        for m in &b.matches {
            match m.next_match_for_winner {
                Some(next) => assert!(next > m.match_index && (next as usize) < 7),
                None => assert_eq!(m.match_index, 6),
            }
        }
        assert_eq!(b.total_rounds(), 3);
    }

    #[test]
    fn a_win_moves_the_winner_on_and_schedules_their_next_game() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(4));
        let applied = b
            .apply_game(
                ID,
                game(&b, 0)?,
                "P0",
                "P3",
                GameOutcome::WhiteWin,
                &seeds(4),
            )
            .context("test setup failed")?;
        assert!(
            matches!(applied, GameApplied::Decided(ref d) if d.winner == "P0" && d.loser == "P3")
        );
        assert_eq!(b.matches[2].player_white.as_deref(), Some("P0"));
        assert!(
            b.matches[2].game_id.is_none(),
            "final waits for its second player"
        );

        b.apply_game(
            ID,
            game(&b, 1)?,
            "P1",
            "P2",
            GameOutcome::BlackWin,
            &seeds(4),
        )
        .context("test setup failed")?;
        assert_eq!(b.matches[2].player_black.as_deref(), Some("P2"));
        assert_eq!(b.matches[2].status, MatchStatus::Active);
        assert_eq!(b.current_round(), Some(1));
        Ok(())
    }

    #[test]
    fn a_drawn_game_is_replayed_with_colours_reversed() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(2));
        let first = game(&b, 0)?;
        let applied = b
            .apply_game(ID, first, "P0", "P1", GameOutcome::Draw, &seeds(2))
            .context("test setup failed")?;
        let m = &b.matches[0];
        assert!(matches!(
            applied,
            GameApplied::Replay {
                match_index: 0,
                game_id: Some(_)
            }
        ));
        assert_eq!(
            (m.player_white.as_deref(), m.player_black.as_deref()),
            (Some("P1"), Some("P0"))
        );
        assert_ne!(m.game_id, Some(first), "a replay needs a fresh game");
        assert!(!m.is_decided());
        assert_eq!(
            b.game_ids(),
            vec![first, m.game_id.context("test setup failed")?]
        );
        Ok(())
    }

    #[test]
    fn after_two_drawn_replays_the_higher_seed_goes_through() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(2));
        // Original game and two replays, all drawn; colours flip each time.
        for (white, black) in [("P0", "P1"), ("P1", "P0"), ("P0", "P1")] {
            let id = game(&b, 0)?;
            b.apply_game(ID, id, white, black, GameOutcome::Draw, &seeds(2))
                .context("test setup failed")?;
        }
        let m = &b.matches[0];
        assert!(m.is_decided());
        assert_eq!(m.winner.as_deref(), Some("P0"));
        assert_eq!(m.result_source, Some(ResultSource::SeedTiebreak));
        assert_eq!(m.drawn_games, 3);
        assert_eq!(b.game_ids().len(), 3, "each game listed once");
        Ok(())
    }

    #[test]
    fn games_with_the_wrong_players_or_ids_are_refused() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(4));
        let id = game(&b, 0)?;
        assert!(matches!(
            b.apply_game(ID, id, "P3", "P0", GameOutcome::WhiteWin, &seeds(4)),
            Err(EliminationError::PlayersMismatch { .. })
        ));
        assert_eq!(
            b.apply_game(ID, 999, "P0", "P3", GameOutcome::WhiteWin, &seeds(4)),
            Err(EliminationError::UnknownGame(999))
        );
        b.apply_game(ID, id, "P0", "P3", GameOutcome::WhiteWin, &seeds(4))
            .context("test setup failed")?;
        assert_eq!(
            b.apply_game(ID, id, "P0", "P3", GameOutcome::WhiteWin, &seeds(4)),
            Err(EliminationError::UnknownGame(id)),
            "a settled game cannot decide its match twice"
        );
        Ok(())
    }

    #[test]
    fn decisions_need_the_match_players_and_an_undecided_match() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(4));
        assert_eq!(
            b.decide(ID, 0, "P1", "P3", ResultSource::Oracle),
            Err(EliminationError::NotInMatch("P1".into(), 0))
        );
        assert_eq!(
            b.decide(ID, 2, "P0", "P1", ResultSource::Oracle),
            Err(EliminationError::SeatsOpen(2))
        );
        assert_eq!(
            b.decide(ID, 9, "P0", "P1", ResultSource::Oracle),
            Err(EliminationError::NoSuchMatch(9))
        );
        b.decide(ID, 0, "P3", "P0", ResultSource::Forfeit)
            .context("test setup failed")?;
        assert_eq!(
            b.decide(ID, 0, "P3", "P0", ResultSource::Forfeit),
            Err(EliminationError::AlreadyDecided(0))
        );
        Ok(())
    }

    #[test]
    fn player_views_follow_the_bracket() -> anyhow::Result<()> {
        let mut b = EliminationState::draw(ID, &seeds(4));
        let nodes = HashMap::new();
        let seat = b
            .assignment_for("P3", &nodes)
            .context("test setup failed")?;
        assert_eq!(
            (
                seat.round,
                seat.your_color.as_str(),
                seat.opponent_pubkey.as_str()
            ),
            (Some(0), "black", "P0")
        );

        b.decide(ID, 0, "P0", "P3", ResultSource::OnChain)
            .context("test setup failed")?;
        assert!(b.is_knocked_out("P3"));
        assert!(
            b.assignment_for("P0", &nodes).is_none(),
            "waiting for the other semifinal"
        );
        let last = b.last_result_for("P0").context("test setup failed")?;
        assert!(last.won && last.opponent == "P3");
        assert_eq!(b.open_matches_in(0), 1);
        assert_eq!(
            b.unsynced().map(|m| m.match_index).collect::<Vec<_>>(),
            vec![0]
        );
        Ok(())
    }
}
