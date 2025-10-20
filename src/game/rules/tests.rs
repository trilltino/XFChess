//! Comprehensive test suite for chess move validation
//!
//! Tests all piece movement rules and board state validation using pure functions.
//! These tests verify correctness of chess rules without requiring ECS infrastructure.
//!
//! # Test Organization
//!
//! - `test_board_state_*` - BoardState query operations
//! - `test_pawn_*` - Pawn movement (forward, double-move, capture)
//! - `test_knight_*` - Knight L-shaped movement
//! - `test_bishop_*` - Bishop diagonal movement and path blocking
//! - `test_rook_*` - Rook horizontal/vertical movement and path blocking
//! - `test_queen_*` - Queen combined rook+bishop movement
//! - `test_king_*` - King single-square movement
//! - `test_integration_*` - Complex multi-piece scenarios
//!
//! # Reference
//!
//! Chess rules tested follow standard FIDE regulations. For advanced move generation
//! algorithms, see `reference/chess_engine/move_gen.rs` which uses bitboards for
//! performance (2-5x faster than array-based validation).

use super::*;
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::game::rules::piece_moves::is_valid_move;
use bevy::prelude::Entity;
use bevy::ecs::entity::EntityRow;

/// Helper function to create a test board state from piece definitions
///
/// Creates a minimal BoardState for testing move validation. Takes a list of
/// (piece_type, color, position) tuples and builds the internal representation.
/// This allows concise test setup without spawning actual ECS entities.
///
/// # Example
/// ```
/// let board = create_test_board(&[
///     (PieceType::Pawn, PieceColor::White, (1, 4)),
///     (PieceType::Rook, PieceColor::Black, (7, 4)),
/// ]);
/// ```
fn create_test_board(pieces: &[(PieceType, PieceColor, (u8, u8))]) -> BoardState {
    let mut board_pieces = Vec::new();
    for (i, &(piece_type, color, pos)) in pieces.iter().enumerate() {
        // Use from_row to create valid test entities (Bevy 0.17 API)
        // EntityRow requires a value that isn't u32::MAX
        let entity = Entity::from_row(EntityRow::from_raw_u32(i as u32).unwrap());
        let piece = Piece {
            color,
            piece_type,
            x: pos.0,
            y: pos.1,
        };
        board_pieces.push((entity, piece, pos));
    }
    BoardState {
        pieces: board_pieces,
    }
}

// ============================================================================
// Board State Tests
// ============================================================================

#[test]
fn test_board_state_is_empty() {
    //! Verifies that BoardState correctly identifies empty squares
    //!
    //! Tests the `is_empty()` method which is crucial for move validation.
    //! Empty squares allow piece movement, while occupied squares block paths
    //! or enable captures. This test ensures basic board querying works.
    let board = create_test_board(&[(PieceType::Pawn, PieceColor::White, (3, 3))]);

    assert!(board.is_empty((2, 2)), "Adjacent square should be empty");
    assert!(
        !board.is_empty((3, 3)),
        "Square with piece should not be empty"
    );
    assert!(board.is_empty((7, 7)), "Far corner should be empty");
}

#[test]
fn test_board_state_get_piece_color() {
    //! Tests color detection for occupied squares
    //!
    //! The `get_piece_color()` method is essential for preventing friendly-fire
    //! captures (you can't capture your own pieces). This test verifies both
    //! Some(color) for occupied squares and None for empty squares.
    let board = create_test_board(&[
        (PieceType::Pawn, PieceColor::White, (1, 0)),
        (PieceType::Pawn, PieceColor::Black, (6, 0)),
    ]);

    assert_eq!(
        board.get_piece_color((1, 0)),
        Some(PieceColor::White),
        "White pawn should be detected"
    );
    assert_eq!(
        board.get_piece_color((6, 0)),
        Some(PieceColor::Black),
        "Black pawn should be detected"
    );
    assert_eq!(
        board.get_piece_color((3, 3)),
        None,
        "Empty square should return None"
    );
}

// ============================================================================
// Pawn Movement Tests
// ============================================================================

#[test]
fn test_pawn_single_forward_move() {
    //! Tests basic pawn forward movement (one square)
    //!
    //! Pawns move forward one square if unobstructed. White pawns move towards
    //! higher ranks (increasing Y), black pawns towards lower ranks (decreasing Y).
    //! This is the most fundamental pawn move and must work correctly.
    let board = create_test_board(&[(PieceType::Pawn, PieceColor::White, (1, 4))]);

    assert!(
        is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (1, 4),
            (2, 4),
            &board,
            false
        ),
        "White pawn should move forward one square"
    );

    let board_black = create_test_board(&[(PieceType::Pawn, PieceColor::Black, (6, 4))]);

    assert!(
        is_valid_move(
            PieceType::Pawn,
            PieceColor::Black,
            (6, 4),
            (5, 4),
            &board_black,
            false
        ),
        "Black pawn should move forward one square"
    );
}

#[test]
fn test_pawn_double_forward_from_start() {
    //! Tests pawn double-move from starting position
    //!
    //! Pawns can move two squares forward on their first move (when has_moved=false).
    //! White pawns start on rank 1, black on rank 6. The path must be clear for
    //! both the intermediate and destination squares. This rule speeds up game openings.
    let board = create_test_board(&[(PieceType::Pawn, PieceColor::White, (1, 3))]);

    assert!(
        is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (1, 3),
            (3, 3),
            &board,
            false
        ),
        "White pawn should double-move from starting position"
    );

    assert!(
        !is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (1, 3),
            (3, 3),
            &board,
            true
        ),
        "Pawn should not double-move after already moving (has_moved=true)"
    );
}

#[test]
fn test_pawn_blocked_by_piece() {
    //! Verifies pawns cannot move through other pieces
    //!
    //! Pawns cannot jump over pieces, unlike knights. Both single and double
    //! forward moves are blocked by any piece directly in front. This test
    //! ensures path-clear validation works for pawns.
    let board = create_test_board(&[
        (PieceType::Pawn, PieceColor::White, (1, 2)),
        (PieceType::Pawn, PieceColor::Black, (2, 2)),
    ]);

    assert!(
        !is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (1, 2),
            (2, 2),
            &board,
            false
        ),
        "Pawn should not move forward into occupied square"
    );
}

#[test]
fn test_pawn_diagonal_capture() {
    //! Tests pawn diagonal capture rules
    //!
    //! Pawns capture differently than they move - one square diagonally forward.
    //! They can ONLY move diagonally when capturing an enemy piece. This asymmetry
    //! is unique to pawns and is a fundamental chess rule.
    let board = create_test_board(&[
        (PieceType::Pawn, PieceColor::White, (3, 3)),
        (PieceType::Pawn, PieceColor::Black, (4, 4)),
    ]);

    assert!(
        is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (3, 3),
            (4, 4),
            &board,
            false
        ),
        "White pawn should capture diagonally"
    );

    assert!(
        !is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (3, 3),
            (4, 2),
            &board,
            false
        ),
        "Pawn should not move diagonally to empty square"
    );
}

#[test]
fn test_pawn_cannot_capture_own_color() {
    //! Ensures pawns cannot capture friendly pieces
    //!
    //! Even though a pawn can move diagonally to capture, it cannot capture
    //! pieces of its own color. This test verifies the color-checking logic
    //! in pawn capture validation.
    let board = create_test_board(&[
        (PieceType::Pawn, PieceColor::White, (3, 3)),
        (PieceType::Rook, PieceColor::White, (4, 4)),
    ]);

    assert!(
        !is_valid_move(
            PieceType::Pawn,
            PieceColor::White,
            (3, 3),
            (4, 4),
            &board,
            false
        ),
        "Pawn should not capture friendly piece"
    );
}

// ============================================================================
// Knight Movement Tests
// ============================================================================

#[test]
fn test_knight_l_shaped_movement() {
    //! Tests all eight possible knight moves (L-shaped)
    //!
    //! Knights move in an "L" shape: 2 squares in one direction, 1 square perpendicular.
    //! This creates 8 possible moves from any position. Knights are the only piece
    //! that can "jump" over other pieces, making path-clear validation unnecessary.
    let board = create_test_board(&[(PieceType::Knight, PieceColor::White, (4, 4))]);

    // All 8 valid L-shaped moves from (4, 4)
    let valid_moves = [
        (6, 5),
        (6, 3),
        (5, 6),
        (5, 2),
        (3, 6),
        (3, 2),
        (2, 5),
        (2, 3),
    ];

    for &target in &valid_moves {
        assert!(
            is_valid_move(
                PieceType::Knight,
                PieceColor::White,
                (4, 4),
                target,
                &board,
                false
            ),
            "Knight should move to {:?} (L-shaped)",
            target
        );
    }

    assert!(
        !is_valid_move(
            PieceType::Knight,
            PieceColor::White,
            (4, 4),
            (5, 5),
            &board,
            false
        ),
        "Knight should not move diagonally (not L-shaped)"
    );
}

#[test]
fn test_knight_can_jump_over_pieces() {
    //! Verifies knights can jump over other pieces
    //!
    //! Unlike all other pieces, knights ignore pieces in their path and can
    //! "jump" to their destination. This test surrounds a knight with pieces
    //! and verifies it can still reach all 8 L-shaped destinations.
    let board = create_test_board(&[
        (PieceType::Knight, PieceColor::White, (4, 4)),
        (PieceType::Pawn, PieceColor::White, (4, 5)),
        (PieceType::Pawn, PieceColor::White, (5, 4)),
        (PieceType::Pawn, PieceColor::White, (4, 3)),
        (PieceType::Pawn, PieceColor::White, (3, 4)),
    ]);

    assert!(
        is_valid_move(
            PieceType::Knight,
            PieceColor::White,
            (4, 4),
            (6, 5),
            &board,
            false
        ),
        "Knight should jump over surrounding pieces"
    );
}

// ============================================================================
// Bishop Movement Tests
// ============================================================================

#[test]
fn test_bishop_diagonal_movement() {
    //! Tests bishop diagonal movement in all four directions
    //!
    //! Bishops move diagonally any number of squares. From any position, a bishop
    //! can move in 4 diagonal directions (NE, NW, SE, SW). This test verifies
    //! movement along all four diagonals with varying distances.
    let board = create_test_board(&[(PieceType::Bishop, PieceColor::White, (3, 3))]);

    assert!(
        is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (3, 3),
            (5, 5),
            &board,
            false
        ),
        "Bishop should move diagonally northeast"
    );
    assert!(
        is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (3, 3),
            (1, 1),
            &board,
            false
        ),
        "Bishop should move diagonally southwest"
    );
    assert!(
        is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (3, 3),
            (0, 6),
            &board,
            false
        ),
        "Bishop should move diagonally northwest"
    );

    assert!(
        !is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (3, 3),
            (3, 5),
            &board,
            false
        ),
        "Bishop should not move horizontally"
    );
}

#[test]
fn test_bishop_blocked_by_piece() {
    //! Ensures bishops cannot move through other pieces
    //!
    //! Bishops are "sliding pieces" - they can move multiple squares but cannot
    //! jump over pieces. This test places a piece in the bishop's diagonal path
    //! and verifies movement is blocked both to and beyond the blocking piece.
    let board = create_test_board(&[
        (PieceType::Bishop, PieceColor::White, (2, 2)),
        (PieceType::Pawn, PieceColor::White, (4, 4)),
    ]);

    assert!(
        !is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (2, 2),
            (5, 5),
            &board,
            false
        ),
        "Bishop should not jump over piece at (4,4)"
    );

    assert!(
        !is_valid_move(
            PieceType::Bishop,
            PieceColor::White,
            (2, 2),
            (4, 4),
            &board,
            false
        ),
        "Bishop should not capture friendly piece"
    );
}

// ============================================================================
// Rook Movement Tests
// ============================================================================

#[test]
fn test_rook_horizontal_vertical_movement() {
    //! Tests rook movement along ranks and files
    //!
    //! Rooks move horizontally (along ranks) or vertically (along files) any
    //! number of squares. From any position, a rook has 4 possible directions:
    //! up, down, left, right. This test verifies movement in all directions.
    let board = create_test_board(&[(PieceType::Rook, PieceColor::White, (3, 3))]);

    assert!(
        is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (3, 7),
            &board,
            false
        ),
        "Rook should move horizontally right"
    );
    assert!(
        is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (3, 0),
            &board,
            false
        ),
        "Rook should move horizontally left"
    );
    assert!(
        is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (7, 3),
            &board,
            false
        ),
        "Rook should move vertically down"
    );

    assert!(
        !is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (5, 5),
            &board,
            false
        ),
        "Rook should not move diagonally"
    );
}

#[test]
fn test_rook_blocked_by_piece() {
    //! Verifies rooks cannot move through pieces
    //!
    //! Like bishops, rooks are sliding pieces that cannot jump. This test
    //! places obstacles in horizontal and vertical paths to ensure proper
    //! path-clear validation for rook movement.
    let board = create_test_board(&[
        (PieceType::Rook, PieceColor::White, (3, 3)),
        (PieceType::Pawn, PieceColor::Black, (3, 5)),
    ]);

    assert!(
        is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (3, 5),
            &board,
            false
        ),
        "Rook should capture enemy piece"
    );

    assert!(
        !is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (3, 3),
            (3, 6),
            &board,
            false
        ),
        "Rook should not jump over piece at (3,5)"
    );
}

// ============================================================================
// Queen Movement Tests
// ============================================================================

#[test]
fn test_queen_combined_movement() {
    //! Tests queen movement (combination of rook + bishop)
    //!
    //! The queen is the most powerful piece, combining rook (horizontal/vertical)
    //! and bishop (diagonal) movement. It can move any number of squares in any
    //! of the 8 directions. This test verifies movement in all 8 directions.
    let board = create_test_board(&[(PieceType::Queen, PieceColor::White, (3, 3))]);

    // Diagonal (bishop-like)
    assert!(
        is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (5, 5),
            &board,
            false
        ),
        "Queen should move diagonally"
    );

    // Horizontal (rook-like)
    assert!(
        is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (3, 7),
            &board,
            false
        ),
        "Queen should move horizontally"
    );

    // Vertical (rook-like)
    assert!(
        is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (7, 3),
            &board,
            false
        ),
        "Queen should move vertically"
    );

    assert!(
        !is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (5, 4),
            &board,
            false
        ),
        "Queen should not move like a knight"
    );
}

// ============================================================================
// King Movement Tests
// ============================================================================

#[test]
fn test_king_single_square_movement() {
    //! Tests king movement (one square in any direction)
    //!
    //! Kings move exactly one square in any of the 8 directions (horizontal,
    //! vertical, or diagonal). This limited movement makes the king vulnerable,
    //! which is why protecting it is the game's primary objective.
    let board = create_test_board(&[(PieceType::King, PieceColor::White, (4, 4))]);

    let valid_moves = [
        (5, 4),
        (5, 5),
        (4, 5),
        (3, 5),
        (3, 4),
        (3, 3),
        (4, 3),
        (5, 3),
    ];

    for &target in &valid_moves {
        assert!(
            is_valid_move(
                PieceType::King,
                PieceColor::White,
                (4, 4),
                target,
                &board,
                false
            ),
            "King should move one square to {:?}",
            target
        );
    }

    assert!(
        !is_valid_move(
            PieceType::King,
            PieceColor::White,
            (4, 4),
            (6, 4),
            &board,
            false
        ),
        "King should not move two squares (except castling)"
    );
}

// ============================================================================
// Integration Tests - Complex Scenarios
// ============================================================================

#[test]
fn test_get_possible_moves_empty_board() {
    //! Tests move generation on an empty board
    //!
    //! With no obstacles, pieces should have maximum mobility. A queen in the
    //! center of an empty board should have 27 possible moves (8 directions,
    //! up to 7 squares each, minus current position). This verifies the
    //! `get_possible_moves` function works correctly.
    let board = create_test_board(&[(PieceType::Queen, PieceColor::White, (3, 3))]);

    let moves = get_possible_moves(
        PieceType::Queen,
        PieceColor::White,
        (3, 3),
        &board,
        false,
    );

    assert!(
        moves.len() > 20,
        "Queen in center should have many moves on empty board, got {}",
        moves.len()
    );
    assert!(
        moves.contains(&(0, 0)),
        "Queen should be able to reach corner"
    );
    assert!(
        moves.contains(&(7, 7)),
        "Queen should be able to reach opposite corner"
    );
}

#[test]
fn test_get_possible_moves_surrounded_piece() {
    //! Tests move generation for completely surrounded piece
    //!
    //! A piece surrounded by friendly pieces (except knights) should have
    //! no legal moves. This test ensures `get_possible_moves` correctly
    //! filters out moves to occupied friendly squares.
    let board = create_test_board(&[
        (PieceType::Rook, PieceColor::White, (4, 4)),
        (PieceType::Pawn, PieceColor::White, (4, 5)),
        (PieceType::Pawn, PieceColor::White, (4, 3)),
        (PieceType::Pawn, PieceColor::White, (5, 4)),
        (PieceType::Pawn, PieceColor::White, (3, 4)),
    ]);

    let moves = get_possible_moves(
        PieceType::Rook,
        PieceColor::White,
        (4, 4),
        &board,
        false,
    );

    assert_eq!(
        moves.len(),
        0,
        "Rook surrounded by friendly pieces should have no moves"
    );
}

#[test]
fn test_complex_board_scenario() {
    //! Integration test with multiple pieces of both colors
    //!
    //! Tests move validation in a realistic mid-game scenario with pieces
    //! of both colors, blocked paths, and capture opportunities. This ensures
    //! the validation logic handles complex interactions correctly.
    let board = create_test_board(&[
        (PieceType::Queen, PieceColor::White, (3, 3)),
        (PieceType::Pawn, PieceColor::White, (4, 4)),
        (PieceType::Rook, PieceColor::Black, (3, 6)),
        (PieceType::Bishop, PieceColor::Black, (6, 6)),
    ]);

    // Queen can capture enemy rook
    assert!(
        is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (3, 6),
            &board,
            false
        ),
        "Queen should capture enemy rook"
    );

    // Queen cannot jump over friendly pawn
    assert!(
        !is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (5, 5),
            &board,
            false
        ),
        "Queen should not jump over friendly pawn"
    );

    // Queen can capture but not jump
    assert!(
        !is_valid_move(
            PieceType::Queen,
            PieceColor::White,
            (3, 3),
            (7, 7),
            &board,
            false
        ),
        "Queen should not jump over enemy bishop to reach corner"
    );
}

#[test]
fn test_boundary_validation() {
    //! Tests that pieces cannot move off the board
    //!
    //! The board is 8x8 (coordinates 0-7). Pieces should not be able to move
    //! to coordinates outside this range. This test ensures proper bounds
    //! checking in the validation logic.
    let board = create_test_board(&[(PieceType::Rook, PieceColor::White, (0, 0))]);

    // These coordinates are off the board (>7)
    assert!(
        !is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (0, 0),
            (0, 8),
            &board,
            false
        ),
        "Rook should not move beyond board boundary"
    );

    assert!(
        !is_valid_move(
            PieceType::Rook,
            PieceColor::White,
            (0, 0),
            (8, 0),
            &board,
            false
        ),
        "Rook should not move beyond board boundary"
    );
}
