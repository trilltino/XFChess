//! Game Flow Integration Tests
//!
//! Tests for full game flows including:
//! - Turn alternation
//! - Piece movement validation
//! - Game state transitions
//! - Win conditions

use bevy::prelude::*;
use chess_engine::api::new_game;
use chess_engine::board::{get_piece_at, is_empty};
use chess_engine::constants::{
    B_KING, B_KNIGHT, B_PAWN, COLOR_BLACK, COLOR_WHITE, W_KING, W_KNIGHT, W_PAWN,
};
use chess_engine::move_gen::generate_pseudo_legal_moves;
use chess_engine::types::Game;

/// Helper to make a move on the game
fn make_move_simple(game: &mut Game, from: i8, to: i8) {
    let piece = game.board[from as usize];
    game.board[to as usize] = piece;
    game.board[from as usize] = 0;
}

// ============================================================================
// Turn Alternation Tests
// ============================================================================

#[test]
fn test_white_moves_first() {
    let game = new_game();

    // White should have moves from starting position
    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);
    assert!(
        !white_moves.is_empty(),
        "White should have moves from starting position"
    );
}

#[test]
fn test_both_players_have_moves() {
    let game = new_game();

    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);
    let black_moves = generate_pseudo_legal_moves(&game, COLOR_BLACK);

    assert_eq!(white_moves.len(), 20, "White should have 20 moves");
    assert_eq!(black_moves.len(), 20, "Black should have 20 moves");
}

// ============================================================================
// Piece Movement Tests
// ============================================================================

#[test]
fn test_pawn_double_push() {
    let game = new_game();
    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);

    // Check that e2-e4 is available (pawn double push)
    let e2_to_e4 = white_moves.iter().any(|m| m.src == 12 && m.dst == 28);
    assert!(e2_to_e4, "e2-e4 should be a valid move");
}

#[test]
fn test_knight_has_two_moves_from_start() {
    let game = new_game();
    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);

    // Knight on b1 (pos 1) should have exactly 2 moves: Na3 and Nc3
    let b1_knight_moves: Vec<_> = white_moves.iter().filter(|m| m.src == 1).collect();

    assert_eq!(b1_knight_moves.len(), 2, "Knight on b1 should have 2 moves");
}

#[test]
fn test_move_changes_board_state() {
    let mut game = new_game();

    // e2 should have a pawn, e4 should be empty
    assert_eq!(game.board[12], W_PAWN, "e2 should have white pawn");
    assert!(is_empty(&game.board, 28), "e4 should be empty");

    // Make move e2-e4
    make_move_simple(&mut game, 12, 28);

    // Now e2 should be empty, e4 should have pawn
    assert!(is_empty(&game.board, 12), "e2 should be empty after move");
    assert_eq!(
        game.board[28], W_PAWN,
        "e4 should have white pawn after move"
    );
}

// ============================================================================
// Capture Tests
// ============================================================================

#[test]
fn test_pawn_can_capture_diagonally() {
    let mut game = new_game();

    // Setup: White pawn on e4, Black pawn on d5
    game.board[28] = W_PAWN; // e4
    game.board[35] = B_PAWN; // d5

    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);

    // Pawn on e4 should be able to capture on d5
    let e4_captures_d5 = white_moves.iter().any(|m| m.src == 28 && m.dst == 35);
    assert!(e4_captures_d5, "e4 pawn should be able to capture on d5");
}

#[test]
fn test_knight_can_capture() {
    let mut game = new_game();

    // Clear board
    for i in 0..64 {
        game.board[i] = 0;
    }

    // White knight on e4, black pawn on f6
    game.board[28] = W_KNIGHT;
    game.board[45] = B_PAWN;

    let white_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);

    // Knight should be able to capture pawn
    let captures_f6 = white_moves.iter().any(|m| m.src == 28 && m.dst == 45);
    assert!(captures_f6, "Knight should be able to capture on f6");
}

// ============================================================================
// Board State Tests
// ============================================================================

#[test]
fn test_initial_king_positions() {
    let game = new_game();

    // Engine uses d1/d8 for kings (non-standard layout)
    assert_eq!(game.board[3], W_KING, "White king should be on d1");
    assert_eq!(game.board[59], B_KING, "Black king should be on d8");
}

#[test]
fn test_piece_count_starting_position() {
    let game = new_game();

    let white_pieces: usize = game.board.iter().filter(|&&p| p > 0).count();
    let black_pieces: usize = game.board.iter().filter(|&&p| p < 0).count();

    assert_eq!(white_pieces, 16, "Should have 16 white pieces");
    assert_eq!(black_pieces, 16, "Should have 16 black pieces");
}

#[test]
fn test_empty_squares_in_middle() {
    let game = new_game();

    // Ranks 3-6 should be empty at start
    for rank in 2..6 {
        for file in 0..8 {
            let pos = rank * 8 + file;
            assert!(is_empty(&game.board, pos), "Square {} should be empty", pos);
        }
    }
}

// ============================================================================
// Move Generation After Game Progression
// ============================================================================

#[test]
fn test_moves_change_after_e4() {
    let mut game = new_game();

    let initial_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE).len();

    // Play e2-e4
    make_move_simple(&mut game, 12, 28);

    let after_e4_moves = generate_pseudo_legal_moves(&game, COLOR_WHITE).len();

    // Move count should change (generally increase as queen/bishop open up)
    assert_ne!(
        initial_moves, after_e4_moves,
        "Move count should change after e4"
    );
}

#[test]
fn test_black_responds_to_e4() {
    let mut game = new_game();

    // White plays e4
    make_move_simple(&mut game, 12, 28);

    // Black should still have moves
    let black_moves = generate_pseudo_legal_moves(&game, COLOR_BLACK);
    assert!(!black_moves.is_empty(), "Black should have moves after e4");

    // e7-e5 should be available
    let e5_available = black_moves.iter().any(|m| m.src == 52 && m.dst == 36);
    assert!(e5_available, "Black should be able to play e5");
}
