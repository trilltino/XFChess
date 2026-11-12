//! Strict-FIDE adjudication for host games: repetition, 50/75-move rules,
//! dead positions and the direction-specific "can the opponent ever mate"
//! test used for flag fall and resignation (FIDE Articles 5.1.2, 5.2.2, 6.9,
//! 9.2, 9.3 and 9.6). A claimable state is never a terminal state.
use crate::Game;
pub use xfchess_adjudication::{
    ClaimAvailability, PositionFacts, PositionKey, RepetitionCheckpoint, Termination,
    FIFTY_MOVE_HALFMOVES, MAX_REVERSIBLE_POSITIONS, SEVENTY_FIVE_MOVE_HALFMOVES,
};
use xfchess_adjudication::{automatic_termination, RepetitionTracker};

/// A verified, ordered legal move sequence and the repetition state it implies.
/// Only `play_uci` advances it, so a render frame or an unverified peer message
/// can never count as an accepted position.
#[derive(Clone, Debug)]
pub struct MatchRecord {
    game: Game,
    keys: Vec<PositionKey>,
    checkpoint: RepetitionCheckpoint,
    moves: Vec<String>,
    terminal: Option<Termination>,
}

impl Default for MatchRecord {
    fn default() -> Self {
        Self::new(Game::default()).unwrap_or_else(|_| Self::empty())
    }
}

impl MatchRecord {
    fn empty() -> Self {
        Self {
            game: Game::default(),
            keys: vec![PositionKey::default(); MAX_REVERSIBLE_POSITIONS],
            checkpoint: RepetitionCheckpoint {
                len: 1,
                ply: 0,
                halfmoves: 0,
            },
            moves: Vec::new(),
            terminal: None,
        }
    }

    pub fn from_fen(fen: &str) -> Result<Self, String> {
        Self::new(Game::from_fen(fen)?)
    }

    /// Start a record from `game`. The position before the first move counts as
    /// its first occurrence.
    pub fn new(game: Game) -> Result<Self, String> {
        let key = game.position_key()?;
        let halfmoves = u16::try_from(game.halfmove_clock())
            .map_err(|_| "halfmove clock out of range".to_string())?;
        let mut keys = vec![PositionKey::default(); MAX_REVERSIBLE_POSITIONS];
        let checkpoint = RepetitionTracker::from_start_position(&mut keys, key, 0, halfmoves)
            .map_err(|e| format!("invalid starting position: {e:?}"))?
            .checkpoint();
        let mut record = Self {
            game,
            keys,
            checkpoint,
            moves: Vec::new(),
            terminal: None,
        };
        record.terminal = record.evaluate()?;
        Ok(record)
    }

    /// Rebuild from an authenticated ordered move list. Every move must be
    /// legal; a single bad move rejects the whole history (fail closed).
    pub fn replay<'a>(
        start_fen: &str,
        moves: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, String> {
        let mut record = Self::from_fen(start_fen)?;
        for uci in moves {
            record.play_uci(uci)?;
        }
        Ok(record)
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn moves(&self) -> &[String] {
        &self.moves
    }

    /// Accepted plies since the record began.
    pub fn ply(&self) -> u32 {
        self.checkpoint.ply
    }

    pub fn halfmoves(&self) -> u16 {
        self.checkpoint.halfmoves
    }

    pub fn terminal(&self) -> Option<Termination> {
        self.terminal
    }

    /// Times the current position has now appeared (initial position counts).
    pub fn occurrences(&self) -> u16 {
        self
            .with_tracker(|tracker| tracker.occurrences(tracker.current_key()))
            .unwrap_or(0)
    }

    /// Claims the side to move may make now. Empty once the game is over.
    pub fn claims(&self) -> ClaimAvailability {
        if self.terminal.is_some() {
            return ClaimAvailability::default();
        }
        self.with_tracker(|tracker| tracker.current_claims())
            .unwrap_or_default()
    }

    /// Claims supported by an irrevocably declared legal intended move. The
    /// move is validated but never applied.
    pub fn claims_after(&self, uci: &str) -> Result<ClaimAvailability, String> {
        if self.terminal.is_some() {
            return Err("game is over".to_string());
        }
        let (next, reset) = self.game.preview_uci(uci)?;
        let key = next.position_key()?;
        self.with_tracker(|tracker| tracker.prospective_claims(key, reset))
            .ok_or_else(|| "corrupt repetition checkpoint".to_string())?
            .map_err(|e| format!("{e:?}"))
    }

    /// Play one legal move; returns the automatic termination it caused, if any.
    /// A rejected move changes nothing.
    pub fn play_uci(&mut self, uci: &str) -> Result<Option<Termination>, String> {
        if self.terminal.is_some() {
            return Err("game is over".to_string());
        }
        let (next, reset) = self.game.preview_uci(uci)?;
        let key = next.position_key()?;
        let ply = self
            .checkpoint
            .ply
            .checked_add(1)
            .ok_or_else(|| "ply overflow".to_string())?;
        let mut keys = self.keys.clone();
        let mut tracker = RepetitionTracker::restore(&mut keys, self.checkpoint)
            .map_err(|e| format!("{e:?}"))?;
        tracker
            .record_accepted_position(key, ply, reset)
            .map_err(|e| format!("{e:?}"))?;
        let checkpoint = tracker.checkpoint();
        let mut staged = self.clone();
        staged.keys = keys;
        staged.checkpoint = checkpoint;
        staged.game = next;
        staged.moves.push(uci.to_string());
        staged.terminal = staged.evaluate()?;
        *self = staged;
        Ok(self.terminal)
    }

    /// Whether `attacker` could checkmate the other side by *any* sequence of
    /// legal moves that the material alone does not rule out (FIDE 5.1.2/6.9).
    /// Material proofs are sound but incomplete: an exotic blocked position is
    /// treated as "can mate", so it can never turn a loss into a draw wrongly.
    pub fn can_possibly_mate(&self, attacker_white: bool) -> bool {
        self.game.can_possibly_mate(attacker_white)
    }

    /// Result of `flagged_white`'s clock reaching zero (Article 6.9).
    pub fn flag_fall(&self, flagged_white: bool) -> Termination {
        if self.can_possibly_mate(!flagged_white) {
            Termination::FlagFall
        } else {
            Termination::FlagFallWithoutPossibleMate
        }
    }

    /// Result of `resigning_white` resigning (Article 5.1.2).
    pub fn resignation(&self, resigning_white: bool) -> Termination {
        if self.can_possibly_mate(!resigning_white) {
            Termination::Resignation
        } else {
            Termination::ResignationWithoutPossibleMate
        }
    }

    fn evaluate(&self) -> Result<Option<Termination>, String> {
        let facts = PositionFacts {
            checkmate: self.game.is_checkmate(),
            stalemate: self.game.is_stalemate(),
            proven_dead: !self.game.can_possibly_mate(true) && !self.game.can_possibly_mate(false),
        };
        Ok(automatic_termination(
            facts,
            self.occurrences(),
            self.checkpoint.halfmoves,
        ))
    }

    /// None only if the checkpoint is corrupt; it is written solely by this type.
    fn with_tracker<T>(&self, f: impl FnOnce(&RepetitionTracker<'_>) -> T) -> Option<T> {
        let mut keys = self.keys.clone();
        RepetitionTracker::restore(&mut keys, self.checkpoint)
            .ok()
            .map(|tracker| f(&tracker))
    }
}

/// What a formal claim by the side to move amounts to (Articles 9.4/9.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// Correct: the game is drawn at once, on the current board.
    Confirmed(Termination),
    /// Authenticated but incorrect: the opponent gains two minutes and, if a
    /// move was declared, that move must be played.
    Incorrect { declared_move: Option<String> },
    /// Not a claim at all (game over, or an illegal or malformed intended
    /// move). Never penalised.
    Rejected(String),
}

impl MatchRecord {
    /// Adjudicate a claim against the verified history without changing it.
    pub fn adjudicate_claim(&self, repetition: bool, intended: Option<&str>) -> ClaimOutcome {
        if self.terminal.is_some() {
            return ClaimOutcome::Rejected("the game is already over".to_string());
        }
        let availability = match intended {
            Some(uci) => match self.claims_after(uci) {
                Ok(availability) => availability,
                Err(error) => return ClaimOutcome::Rejected(format!("intended move: {error}")),
            },
            None => self.claims(),
        };
        let supported = if repetition {
            availability.repetition
        } else {
            availability.fifty_moves
        };
        if supported {
            ClaimOutcome::Confirmed(if repetition {
                Termination::ThreefoldClaim
            } else {
                Termination::FiftyMoveClaim
            })
        } else {
            ClaimOutcome::Incorrect {
                declared_move: intended.map(str::to_string),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNIGHTS: [&str; 4] = ["g1f3", "g8f6", "f3g1", "f6g8"];

    fn cycle(record: &mut MatchRecord, plies: usize) -> Option<Termination> {
        let mut last = None;
        for ply in 0..plies {
            last = record.play_uci(KNIGHTS[ply % 4]).unwrap_or(None);
        }
        last
    }

    #[test]
    fn third_occurrence_is_claimable_but_not_terminal_and_fifth_is_automatic() {
        let mut record = MatchRecord::default();
        assert_eq!(record.occurrences(), 1);
        assert!(cycle(&mut record, 8).is_none());
        assert_eq!(record.occurrences(), 3);
        assert!(record.claims().repetition);
        assert!(record.terminal().is_none());
        assert_eq!(cycle(&mut record, 8), Some(Termination::FivefoldRepetition));
        assert_eq!(record.terminal(), Some(Termination::FivefoldRepetition));
        assert_eq!(record.claims(), ClaimAvailability::default());
        assert!(record.play_uci("e2e4").is_err());
    }

    #[test]
    fn prospective_claim_does_not_change_the_record() {
        let mut record = MatchRecord::default();
        assert!(cycle(&mut record, 7).is_none());
        let before = (record.ply(), record.game().fen());
        let claim = record.claims_after("f6g8");
        assert!(claim.is_ok_and(|c| c.repetition));
        assert!(record.claims_after("e7e5").is_ok_and(|c| !c.repetition));
        assert!(record.claims_after("a1a8").is_err());
        assert_eq!(before, (record.ply(), record.game().fen()));
    }

    #[test]
    fn lost_castling_rights_are_not_the_same_position() {
        let mut record = MatchRecord::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1")
            .unwrap_or_default();
        // Rook out and back changes rights on the second cycle: never 3 equal.
        for uci in ["h1h2", "h8h7", "h2h1", "h7h8", "h1h2", "h8h7", "h2h1", "h7h8"] {
            assert!(record.play_uci(uci).is_ok());
        }
        assert!(!record.claims().repetition);
    }

    #[test]
    fn the_hundredth_halfmove_continues_and_the_hundred_fiftieth_ends_the_game() {
        let mut record = MatchRecord::from_fen("8/8/4k3/8/8/4K3/8/R7 w - - 99 80")
            .unwrap_or_default();
        assert!(!record.claims().fifty_moves);
        assert_eq!(record.play_uci("a1a2").unwrap_or(Some(Termination::Stalemate)), None);
        assert_eq!(record.halfmoves(), 100);
        assert!(record.terminal().is_none());
        assert!(record.claims().fifty_moves);
        assert!(record.claims_after("e6d6").is_ok_and(|c| c.fifty_moves));
        let mut record = MatchRecord::from_fen("8/8/4k3/8/8/4K3/8/R7 w - - 149 90")
            .unwrap_or_default();
        assert_eq!(
            record.play_uci("a1a2").unwrap_or(None),
            Some(Termination::SeventyFiveMoves)
        );
    }

    #[test]
    fn checkmate_on_the_hundred_fiftieth_halfmove_wins() {
        let mut record = MatchRecord::from_fen("7k/8/5KQ1/8/8/8/8/8 w - - 149 90")
            .unwrap_or_default();
        assert_eq!(record.play_uci("g6g7").unwrap_or(None), Some(Termination::Checkmate));
    }

    #[test]
    fn flag_fall_and_resignation_are_direction_specific() {
        // White has a lone knight: black cannot be mated by... black has only a king.
        let record = MatchRecord::from_fen("4k3/8/8/8/8/8/8/4KN2 w - - 0 1").unwrap_or_default();
        assert!(record.can_possibly_mate(true) == false);
        // Black flagging cannot lose to a lone minor piece; white flagging can't
        // be mated by the bare king either.
        assert_eq!(record.flag_fall(true), Termination::FlagFallWithoutPossibleMate);
        let rook = MatchRecord::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1").unwrap_or_default();
        assert_eq!(rook.flag_fall(false), Termination::FlagFall);
        assert_eq!(rook.flag_fall(true), Termination::FlagFallWithoutPossibleMate);
        assert_eq!(rook.resignation(true), Termination::ResignationWithoutPossibleMate);
        assert_eq!(rook.resignation(false), Termination::Resignation);
    }

    #[test]
    fn bare_kings_are_a_dead_position_at_creation() {
        let record = MatchRecord::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap_or_default();
        assert_eq!(record.terminal(), Some(Termination::DeadPosition));
    }

    #[test]
    fn a_rejected_move_changes_nothing() {
        let mut record = MatchRecord::default();
        let before = record.game().fen();
        assert!(record.play_uci("e2e5").is_err());
        assert!(record.play_uci("e7e5").is_err());
        assert_eq!(record.game().fen(), before);
        assert_eq!(record.ply(), 0);
    }

    #[test]
    fn replay_matches_live_play() {
        let mut live = MatchRecord::default();
        cycle(&mut live, 10);
        let replayed = MatchRecord::replay(
            crate::STARTING_FEN,
            live.moves().iter().map(String::as_str),
        );
        assert!(replayed.is_ok_and(|r| r.occurrences() == live.occurrences()
            && r.halfmoves() == live.halfmoves()
            && r.game().fen() == live.game().fen()));
        assert!(MatchRecord::replay(crate::STARTING_FEN, ["g1f3", "e2e4"]).is_err());
    }
}

#[cfg(test)]
mod claim_tests {
    use super::*;

    fn knight_dance(record: &mut MatchRecord, plies: usize) {
        for ply in 0..plies {
            let _ = record.play_uci(["g1f3", "g8f6", "f3g1", "f6g8"][ply % 4]);
        }
    }

    #[test]
    fn a_correct_current_or_prospective_repetition_claim_is_confirmed() {
        let mut record = MatchRecord::default();
        knight_dance(&mut record, 8);
        assert_eq!(
            record.adjudicate_claim(true, None),
            ClaimOutcome::Confirmed(Termination::ThreefoldClaim)
        );
        let mut record = MatchRecord::default();
        knight_dance(&mut record, 6);
        // Third appearance only after the declared g1f3... which is move 7.
        assert!(matches!(
            record.adjudicate_claim(true, None),
            ClaimOutcome::Incorrect { .. }
        ));
    }

    #[test]
    fn an_incorrect_claim_carries_the_declared_move_and_never_mutates() {
        let record = MatchRecord::default();
        let before = record.game().fen();
        assert_eq!(
            record.adjudicate_claim(true, Some("g1f3")),
            ClaimOutcome::Incorrect {
                declared_move: Some("g1f3".to_string())
            }
        );
        assert_eq!(record.game().fen(), before);
    }

    #[test]
    fn malformed_or_illegal_intents_and_finished_games_are_not_claims() {
        let record = MatchRecord::default();
        assert!(matches!(
            record.adjudicate_claim(false, Some("e2e5")),
            ClaimOutcome::Rejected(_)
        ));
        assert!(matches!(
            record.adjudicate_claim(false, Some("zz")),
            ClaimOutcome::Rejected(_)
        ));
        let mut over = MatchRecord::default();
        knight_dance(&mut over, 16);
        assert!(matches!(
            over.adjudicate_claim(true, None),
            ClaimOutcome::Rejected(_)
        ));
    }

    #[test]
    fn a_fifty_move_claim_needs_one_hundred_halfmoves_current_or_after_the_intent() {
        let at_99 = MatchRecord::from_fen("8/8/4k3/8/8/4K3/8/R7 w - - 99 80").unwrap_or_default();
        assert!(matches!(
            at_99.adjudicate_claim(false, None),
            ClaimOutcome::Incorrect { .. }
        ));
        assert_eq!(
            at_99.adjudicate_claim(false, Some("a1a2")),
            ClaimOutcome::Confirmed(Termination::FiftyMoveClaim)
        );
        let at_100 = MatchRecord::from_fen("8/8/4k3/8/8/4K3/8/R7 w - - 100 80").unwrap_or_default();
        assert_eq!(
            at_100.adjudicate_claim(false, None),
            ClaimOutcome::Confirmed(Termination::FiftyMoveClaim)
        );
        // A capture or pawn move resets the count, so it cannot support a claim.
        let pawn = MatchRecord::from_fen("8/8/4k3/8/8/P3K3/8/8 w - - 100 80").unwrap_or_default();
        assert!(matches!(
            pawn.adjudicate_claim(false, Some("a3a4")),
            ClaimOutcome::Incorrect { .. }
        ));
    }
}
