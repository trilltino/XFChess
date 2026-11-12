use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

#[derive(Resource, Default, Debug, Reflect, PartialEq, Eq, Clone, Copy)]
#[reflect(Resource)]
pub enum GameOverState {
    #[default]
    Playing,

    WhiteWon,

    BlackWon,

    Stalemate,

    InsufficientMaterial,

    /// Terminal only after a fifty-move claim was authoritatively accepted.
    FiftyMoveRule,

    /// A threefold-repetition claim was authoritatively accepted.
    ThreefoldRepetition,

    /// Automatic: the same position appeared five times.
    FivefoldRepetition,

    /// Automatic: 75 moves by each side without a pawn move or capture.
    SeventyFiveMoveRule,

    /// No sequence of legal moves lets either side checkmate.
    DeadPosition,

    /// A clock expired but the opponent cannot possibly checkmate (Art. 6.9).
    TimeoutNoMatingMaterial,

    /// A resignation where the opponent cannot possibly checkmate (Art. 5.1.2).
    ResignationNoMatingMaterial,

    /// A draw offer was accepted after both players had moved (Art. 5.2.3).
    DrawByAgreement,

    WhiteWonByTime,

    BlackWonByTime,

    WhiteWonByResignation,

    BlackWonByResignation,

    Aborted,

    WhiteWonByAbandonment,

    BlackWonByAbandonment,
}

impl GameOverState {
    /// Terminal draws the rules end by themselves. Checkmate and stalemate are
    /// read from the legal-move cache, so they map to `None` here.
    pub fn from_automatic(termination: xfchess_rules::Termination) -> Option<Self> {
        use xfchess_rules::Termination as T;
        match termination {
            T::DeadPosition => Some(Self::DeadPosition),
            T::FivefoldRepetition => Some(Self::FivefoldRepetition),
            T::SeventyFiveMoves => Some(Self::SeventyFiveMoveRule),
            _ => None,
        }
    }

    /// The terminal state of an authoritatively accepted draw claim.
    pub fn from_claim(repetition: bool) -> Self {
        if repetition {
            Self::ThreefoldRepetition
        } else {
            Self::FiftyMoveRule
        }
    }

    /// A flag fall (Article 6.9): a draw when `flagged_white`'s opponent could
    /// never mate, otherwise a loss on time.
    pub fn from_flag_fall(record: &xfchess_rules::MatchRecord, flagged_white: bool) -> Self {
        match record.flag_fall(flagged_white) {
            xfchess_rules::Termination::FlagFallWithoutPossibleMate => Self::TimeoutNoMatingMaterial,
            _ if flagged_white => Self::BlackWonByTime,
            _ => Self::WhiteWonByTime,
        }
    }

    /// A resignation (Article 5.1.2): a draw when the opponent could never mate.
    pub fn from_resignation(record: &xfchess_rules::MatchRecord, resigning_white: bool) -> Self {
        match record.resignation(resigning_white) {
            xfchess_rules::Termination::ResignationWithoutPossibleMate => {
                Self::ResignationNoMatingMaterial
            }
            _ if resigning_white => Self::BlackWonByResignation,
            _ => Self::WhiteWonByResignation,
        }
    }

    pub fn is_game_over(&self) -> bool {
        !matches!(self, GameOverState::Playing)
    }

    pub fn message(&self) -> &str {
        match self {
            GameOverState::Playing => "Game in progress",
            GameOverState::WhiteWon => "White wins by checkmate!",
            GameOverState::BlackWon => "Black wins by checkmate!",
            GameOverState::Stalemate => "Draw by stalemate",
            GameOverState::InsufficientMaterial => "Draw by insufficient material",
            GameOverState::FiftyMoveRule => "Draw by the fifty-move rule",
            GameOverState::ThreefoldRepetition => "Draw by threefold repetition",
            GameOverState::FivefoldRepetition => "Draw by fivefold repetition",
            GameOverState::SeventyFiveMoveRule => "Draw by the seventy-five-move rule",
            GameOverState::DeadPosition => "Draw: dead position",
            GameOverState::TimeoutNoMatingMaterial => "Draw: time expired, no possible mate",
            GameOverState::ResignationNoMatingMaterial => "Draw: resignation, no possible mate",
            GameOverState::DrawByAgreement => "Draw by agreement",
            GameOverState::WhiteWonByTime => "White wins on time!",
            GameOverState::BlackWonByTime => "Black wins on time!",
            GameOverState::WhiteWonByResignation => "White wins by resignation!",
            GameOverState::BlackWonByResignation => "Black wins by resignation!",
            GameOverState::Aborted => "Game aborted",
            GameOverState::WhiteWonByAbandonment => "White wins — Black disconnected!",
            GameOverState::BlackWonByAbandonment => "Black wins — White disconnected!",
        }
    }

    pub fn winner(&self) -> Option<PieceColor> {
        match self {
            GameOverState::WhiteWon
            | GameOverState::WhiteWonByTime
            | GameOverState::WhiteWonByResignation => Some(PieceColor::White),
            GameOverState::BlackWon
            | GameOverState::BlackWonByTime
            | GameOverState::BlackWonByResignation => Some(PieceColor::Black),
            GameOverState::WhiteWonByAbandonment => Some(PieceColor::White),
            GameOverState::BlackWonByAbandonment => Some(PieceColor::Black),
            _ => None,
        }
    }

    pub fn is_checkmate(&self) -> bool {
        matches!(self, GameOverState::WhiteWon | GameOverState::BlackWon)
    }

    pub fn is_resignation(&self) -> bool {
        matches!(
            self,
            GameOverState::WhiteWonByResignation | GameOverState::BlackWonByResignation
        )
    }

    pub fn is_draw(&self) -> bool {
        matches!(
            self,
            GameOverState::Stalemate
                | GameOverState::InsufficientMaterial
                | GameOverState::FiftyMoveRule
                | GameOverState::ThreefoldRepetition
                | GameOverState::FivefoldRepetition
                | GameOverState::SeventyFiveMoveRule
                | GameOverState::DeadPosition
                | GameOverState::TimeoutNoMatingMaterial
                | GameOverState::ResignationNoMatingMaterial
                | GameOverState::DrawByAgreement
        )
    }

    pub fn termination_text(&self) -> &str {
        match self {
            GameOverState::WhiteWon | GameOverState::BlackWon => "by checkmate",
            GameOverState::WhiteWonByResignation | GameOverState::BlackWonByResignation => {
                "by resignation"
            }
            GameOverState::WhiteWonByTime | GameOverState::BlackWonByTime => "on timeout",
            GameOverState::Stalemate => "by stalemate",
            GameOverState::InsufficientMaterial => "insufficient material",
            GameOverState::FiftyMoveRule => "fifty-move rule (claimed)",
            GameOverState::ThreefoldRepetition => "threefold repetition (claimed)",
            GameOverState::FivefoldRepetition => "fivefold repetition",
            GameOverState::SeventyFiveMoveRule => "seventy-five-move rule",
            GameOverState::DeadPosition => "dead position",
            GameOverState::TimeoutNoMatingMaterial => "time expired, no possible mate",
            GameOverState::ResignationNoMatingMaterial => "resignation, no possible mate",
            GameOverState::DrawByAgreement => "agreement",
            GameOverState::Aborted => "White didn't move in time",
            GameOverState::WhiteWonByAbandonment | GameOverState::BlackWonByAbandonment => {
                "opponent disconnected"
            }
            GameOverState::Playing => "",
        }
    }
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingGameOver {
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_over_state_default() {
        let state = GameOverState::default();
        assert_eq!(state, GameOverState::Playing);
        assert!(!state.is_game_over());
    }

    #[test]
    fn test_is_game_over_playing() {
        let state = GameOverState::Playing;
        assert!(!state.is_game_over());
    }

    #[test]
    fn test_is_game_over_white_won() {
        let state = GameOverState::WhiteWon;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_black_won() {
        let state = GameOverState::BlackWon;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_stalemate() {
        let state = GameOverState::Stalemate;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_insufficient_material() {
        let state = GameOverState::InsufficientMaterial;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_timeout_white() {
        let state = GameOverState::WhiteWonByTime;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_resignation_black() {
        let state = GameOverState::BlackWonByResignation;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_is_game_over_timeout_black() {
        let state = GameOverState::BlackWonByTime;
        assert!(state.is_game_over());
    }

    #[test]
    fn test_message_playing() {
        let state = GameOverState::Playing;
        assert_eq!(state.message(), "Game in progress");
    }

    #[test]
    fn test_message_white_won() {
        let state = GameOverState::WhiteWon;
        assert_eq!(state.message(), "White wins by checkmate!");
    }

    #[test]
    fn test_message_white_won_by_resignation() {
        let state = GameOverState::WhiteWonByResignation;
        assert_eq!(state.message(), "White wins by resignation!");
    }

    #[test]
    fn test_message_black_won() {
        let state = GameOverState::BlackWon;
        assert_eq!(state.message(), "Black wins by checkmate!");
    }

    #[test]
    fn test_message_stalemate() {
        let state = GameOverState::Stalemate;
        assert_eq!(state.message(), "Draw by stalemate");
    }

    #[test]
    fn test_message_insufficient_material() {
        let state = GameOverState::InsufficientMaterial;
        assert_eq!(state.message(), "Draw by insufficient material");
    }

    #[test]
    fn test_message_white_won_by_time() {
        let state = GameOverState::WhiteWonByTime;
        assert_eq!(state.message(), "White wins on time!");
    }

    #[test]
    fn test_message_black_won_by_time() {
        let state = GameOverState::BlackWonByTime;
        assert_eq!(state.message(), "Black wins on time!");
    }

    #[test]
    fn test_winner_white_checkmate() {
        let state = GameOverState::WhiteWon;
        assert_eq!(state.winner(), Some(PieceColor::White));
    }

    #[test]
    fn test_winner_black_checkmate() {
        let state = GameOverState::BlackWon;
        assert_eq!(state.winner(), Some(PieceColor::Black));
    }

    #[test]
    fn test_winner_white_timeout() {
        let state = GameOverState::WhiteWonByTime;
        assert_eq!(state.winner(), Some(PieceColor::White));
    }

    #[test]
    fn test_winner_black_timeout() {
        let state = GameOverState::BlackWonByTime;
        assert_eq!(state.winner(), Some(PieceColor::Black));
    }

    #[test]
    fn test_winner_stalemate() {
        let state = GameOverState::Stalemate;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_winner_insufficient_material() {
        let state = GameOverState::InsufficientMaterial;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_winner_playing() {
        let state = GameOverState::Playing;
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn test_is_checkmate_white_won() {
        let state = GameOverState::WhiteWon;
        assert!(state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_black_won() {
        let state = GameOverState::BlackWon;
        assert!(state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_timeout() {
        let state = GameOverState::WhiteWonByTime;
        assert!(!state.is_checkmate());
    }

    #[test]
    fn test_is_checkmate_draw() {
        let state = GameOverState::Stalemate;
        assert!(!state.is_checkmate());
    }

    #[test]
    fn test_game_over_state_equality() {
        assert_eq!(GameOverState::Playing, GameOverState::Playing);
        assert_eq!(GameOverState::WhiteWon, GameOverState::WhiteWon);
        assert_ne!(GameOverState::WhiteWon, GameOverState::BlackWon);
        assert_ne!(
            GameOverState::Stalemate,
            GameOverState::InsufficientMaterial
        );
    }

    #[test]
    fn test_aborted_is_game_over_with_no_winner_or_message() {
        let state = GameOverState::Aborted;
        assert!(state.is_game_over());
        assert_eq!(state.winner(), None);
        assert!(!state.is_checkmate());
        assert!(!state.is_resignation());
        assert!(!state.is_draw());
        assert_eq!(state.message(), "Game aborted");
    }

    #[test]
    fn test_game_over_state_clone() {
        let original = GameOverState::Stalemate;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_over_state_copy() {
        let original = GameOverState::WhiteWon;
        let copied = original; // Copy, not move
        assert_eq!(original, copied);
        assert_eq!(original, GameOverState::WhiteWon); // Original still accessible
    }

    #[test]
    fn test_all_end_states_are_game_over() {
        let states = vec![
            GameOverState::WhiteWon,
            GameOverState::BlackWon,
            GameOverState::Stalemate,
            GameOverState::InsufficientMaterial,
            GameOverState::FiftyMoveRule,
            GameOverState::WhiteWonByTime,
            GameOverState::BlackWonByTime,
        ];

        for state in states {
            assert!(state.is_game_over(), "{:?} should be game over", state);
        }
    }

    #[test]
    fn fifty_move_rule_is_a_draw_with_no_winner() {
        let state = GameOverState::FiftyMoveRule;
        assert!(state.is_draw());
        assert_eq!(state.winner(), None);
        assert_eq!(state.message(), "Draw by the fifty-move rule");
    }
}
