//! Components for chess phases and move history.
//!
//! `GamePhase` is distinct from [`crate::core::GameState`], which controls
//! application-level state rather than the phase of an active chess game.

use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::prelude::*;

/// Current phase of an active chess game.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum GamePhase {
    /// Initial board setup phase.
    #[default]
    Setup,

    /// Active gameplay with no check condition.
    Playing,

    /// Current player's king is under attack.
    Check,

    /// Game over: current player is in check with no legal moves.
    Checkmate,

    /// Game over: current player has no legal moves but is not in check.
    Stalemate,
}

/// Record of a chess move and its special-move status.
#[derive(Clone, Copy, Debug, Reflect)]
pub struct MoveRecord {
    pub piece_type: PieceType,

    pub piece_color: PieceColor,

    pub from: (u8, u8),

    pub to: (u8, u8),

    pub captured: Option<PieceType>,

    pub is_castling: bool,

    pub is_en_passant: bool,

    pub is_check: bool,

    pub is_checkmate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_phase_default() {
        //! Verifies GamePhase defaults to Setup

        let phase = GamePhase::default();
        assert_eq!(phase, GamePhase::Setup);
    }

    #[test]
    fn test_game_phase_transitions() {
        //! Tests all valid GamePhase values

        let phases = vec![
            GamePhase::Setup,
            GamePhase::Playing,
            GamePhase::Check,
            GamePhase::Checkmate,
            GamePhase::Stalemate,
        ];

        // All phases should be distinct
        for (i, phase1) in phases.iter().enumerate() {
            for (j, phase2) in phases.iter().enumerate() {
                if i == j {
                    assert_eq!(phase1, phase2);
                } else {
                    assert_ne!(phase1, phase2);
                }
            }
        }
    }

    #[test]
    fn test_game_phase_clone() {
        //! Tests GamePhase can be cloned

        let original = GamePhase::Check;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_game_phase_copy() {
        //! Verifies GamePhase implements Copy

        let original = GamePhase::Playing;
        let copied = original; // Copy, not move
        assert_eq!(original, copied);
        assert_eq!(original, GamePhase::Playing); // Still accessible
    }

    #[test]
    fn test_game_phase_debug() {
        //! Tests debug formatting is useful

        assert_eq!(format!("{:?}", GamePhase::Check), "Check");
        assert_eq!(format!("{:?}", GamePhase::Checkmate), "Checkmate");
    }

    #[test]
    fn test_game_phase_equality() {
        //! Tests PartialEq implementation

        assert_eq!(GamePhase::Playing, GamePhase::Playing);
        assert_ne!(GamePhase::Check, GamePhase::Checkmate);
    }

    #[test]
    fn test_move_record_creation() {
        //! Tests creating a basic move record

        let move_rec = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::White,
            from: (4, 1),
            to: (4, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        assert_eq!(move_rec.piece_type, PieceType::Pawn);
        assert_eq!(move_rec.from, (4, 1));
        assert_eq!(move_rec.to, (4, 3));
        assert!(move_rec.captured.is_none());
    }

    #[test]
    fn test_move_record_capture() {
        //! Tests move record with capture

        let capture_move = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::Black,
            from: (3, 7),
            to: (7, 3),
            captured: Some(PieceType::Rook),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: false,
        };

        assert!(capture_move.captured.is_some());
        assert_eq!(capture_move.captured.unwrap(), PieceType::Rook);
        assert!(capture_move.is_check);
    }

    #[test]
    fn test_move_record_castling() {
        //! Tests castling move record

        let castling_move = MoveRecord {
            piece_type: PieceType::King,
            piece_color: PieceColor::White,
            from: (4, 0),
            to: (6, 0),
            captured: None,
            is_castling: true,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        assert!(castling_move.is_castling);
        assert_eq!(castling_move.piece_type, PieceType::King);
    }

    #[test]
    fn test_move_record_en_passant() {
        //! Tests en passant move record

        let en_passant = MoveRecord {
            piece_type: PieceType::Pawn,
            piece_color: PieceColor::Black,
            from: (4, 3),
            to: (3, 2),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: true,
            is_check: false,
            is_checkmate: false,
        };

        assert!(en_passant.is_en_passant);
        assert_eq!(en_passant.captured, Some(PieceType::Pawn));
    }

    #[test]
    fn test_move_record_checkmate() {
        //! Tests checkmate move record

        let checkmate = MoveRecord {
            piece_type: PieceType::Queen,
            piece_color: PieceColor::White,
            from: (3, 4),
            to: (5, 6),
            captured: Some(PieceType::Pawn),
            is_castling: false,
            is_en_passant: false,
            is_check: true,
            is_checkmate: true,
        };

        assert!(checkmate.is_checkmate);
        assert!(checkmate.is_check); // Checkmate implies check
    }

    #[test]
    fn test_move_record_clone() {
        //! Tests MoveRecord can be cloned

        let original = MoveRecord {
            piece_type: PieceType::Knight,
            piece_color: PieceColor::Black,
            from: (1, 0),
            to: (2, 2),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let cloned = original.clone();
        assert_eq!(original.piece_type, cloned.piece_type);
        assert_eq!(original.from, cloned.from);
    }

    #[test]
    fn test_move_record_copy() {
        //! Tests MoveRecord implements Copy

        let original = MoveRecord {
            piece_type: PieceType::Bishop,
            piece_color: PieceColor::White,
            from: (2, 0),
            to: (5, 3),
            captured: None,
            is_castling: false,
            is_en_passant: false,
            is_check: false,
            is_checkmate: false,
        };

        let copied = original; // Copy, not move
        assert_eq!(original.from, copied.from); // Original still accessible
    }
}
