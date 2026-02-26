//! Game Flow Integration Tests
//!
//! Tests for full game flows including:
//! - Turn alternation (using shakmaty)
//! - Piece movement validation
//! - Starting position sanity checks
//! - Win condition detection

use bevy::prelude::*;
use shakmaty::fen::Fen;
use shakmaty::{Chess, Color, Move, Position, Role, Square};

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn new_game() -> Chess {
    let fen: Fen = START_FEN.parse().unwrap();
    fen.into_position(shakmaty::CastlingMode::Standard).unwrap()
}

// ============================================================================
// Turn Alternation Tests
// ============================================================================

#[test]
fn test_white_moves_first() {
    let game = new_game();
    assert_eq!(game.turn(), Color::White, "White should move first");
    let moves = game.legal_moves();
    assert!(
        !moves.is_empty(),
        "White should have moves from starting position"
    );
}

#[test]
fn test_both_players_have_moves() {
    let game = new_game();
    let white_moves = game.legal_moves();
    assert_eq!(
        white_moves.len(),
        20,
        "White should have 20 legal moves at start"
    );

    // Flip the turn to check black (shakmaty position is immutable, black also has 20 moves
    // from the mirror starting position)
    let fen_black: Fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"
        .parse()
        .unwrap();
    let game_black: Chess = fen_black
        .into_position(shakmaty::CastlingMode::Standard)
        .unwrap();
    let black_moves = game_black.legal_moves();
    assert_eq!(
        black_moves.len(),
        20,
        "Black should have 20 legal moves at start"
    );
}

// ============================================================================
// Piece Movement Tests
// ============================================================================

#[test]
fn test_pawn_double_push_available() {
    let game = new_game();
    let moves = game.legal_moves();
    // e2-e4: from Square::E2 to Square::E4
    let has_e4 = moves
        .iter()
        .any(|m| m.from() == Some(Square::E2) && m.to() == Square::E4);
    assert!(has_e4, "e2-e4 should be a valid move");
}

#[test]
fn test_knight_has_two_moves_from_start() {
    let game = new_game();
    let moves = game.legal_moves();
    let b1_knight_moves: Vec<_> = moves
        .iter()
        .filter(|m| m.from() == Some(Square::B1))
        .collect();
    assert_eq!(
        b1_knight_moves.len(),
        2,
        "Knight on b1 should have 2 moves (Na3, Nc3)"
    );
}

#[test]
fn test_move_changes_position() {
    let game = new_game();
    let moves = game.legal_moves();
    // Find e2-e4
    let e4_move = moves
        .iter()
        .find(|m| m.from() == Some(Square::E2) && m.to() == Square::E4)
        .expect("e2-e4 must be available");
    let new_pos = game.clone().play(e4_move).expect("e2-e4 must be playable");
    // It's now black's turn
    assert_eq!(new_pos.turn(), Color::Black);
    // e4 should have a white pawn
    assert_eq!(new_pos.board().role_at(Square::E4), Some(Role::Pawn));
    assert_eq!(new_pos.board().color_at(Square::E4), Some(Color::White));
}

#[test]
fn test_pawn_can_capture_diagonally() {
    // Build a position with white pawn e4, black pawn d5
    let fen: Fen = "8/8/8/3p4/4P3/8/8/8 w - - 0 1".parse().unwrap();
    let game: Chess = fen.into_position(shakmaty::CastlingMode::Standard).unwrap();
    let moves = game.legal_moves();
    let can_capture = moves
        .iter()
        .any(|m| m.from() == Some(Square::E4) && m.to() == Square::D5);
    assert!(can_capture, "e4 pawn should be able to capture on d5");
}

#[test]
fn test_piece_count_starting_position() {
    let game = new_game();
    let board = game.board();
    let white_pieces = board.white().count();
    let black_pieces = board.black().count();
    assert_eq!(white_pieces, 16, "Should have 16 white pieces");
    assert_eq!(black_pieces, 16, "Should have 16 black pieces");
}

#[test]
fn test_black_responds_to_e4() {
    let game = new_game();
    let moves = game.legal_moves();
    let e4 = moves
        .iter()
        .find(|m| m.from() == Some(Square::E2) && m.to() == Square::E4)
        .unwrap();
    let pos2 = game.play(e4).unwrap();
    assert_eq!(pos2.turn(), Color::Black);
    let black_moves = pos2.legal_moves();
    assert!(!black_moves.is_empty(), "Black should have moves after e4");
    let e5 = black_moves
        .iter()
        .any(|m| m.from() == Some(Square::E7) && m.to() == Square::E5);
    assert!(e5, "Black should be able to play e7-e5");
}

#[test]
fn test_empty_squares_starting_position() {
    let game = new_game();
    let board = game.board();
    // Ranks 3-6 (0-indexed: squares 16..=47 in index, i.e. files a-h, ranks 3-6)
    use shakmaty::Rank;
    for rank in [Rank::Third, Rank::Fourth, Rank::Fifth, Rank::Sixth] {
        for sq in Square::all() {
            if sq.rank() == rank {
                assert!(
                    board.piece_at(sq).is_none(),
                    "Square {:?} should be empty at start",
                    sq
                );
            }
        }
    }
}
