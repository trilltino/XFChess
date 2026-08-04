//! Integration tests for type utilities extracted from doc tests
//!
//! These tests demonstrate usage patterns for File, Rank, Square, and Centipawns types.
//! Original location: src/game/types.rs

use xfchess::game::types::{Centipawns, File, Rank, Square};
use xfchess::rendering::pieces::PieceType;

// File tests

/// Example: Creating a file from a character
/// Original: File::from_char() doc example
#[test]
fn example_file_from_char() {
    if let Some(file) = File::from_char('e') {
        assert_eq!(file.index(), 4); // file is File(4)
    } else {
        panic!("Should have parsed 'e' successfully");
    }

    // Invalid characters return None
    assert!(File::from_char('z').is_none());
}

/// Example: Converting file to character
/// Original: File::to_char() doc example
#[test]
fn example_file_to_char() {
    let file = File(4);
    assert_eq!(file.to_char(), 'e');

    // Test all files
    assert_eq!(File(0).to_char(), 'a');
    assert_eq!(File(7).to_char(), 'h');
}

// Rank tests

/// Example: Creating a rank from a number
/// Original: Rank::from_number() doc example
#[test]
fn example_rank_from_number() {
    let rank = Rank::from_number(4).unwrap(); // Rank 3 (0-indexed)
    assert_eq!(rank.index(), 3);

    // Test boundaries
    assert!(Rank::from_number(1).is_some());
    assert!(Rank::from_number(8).is_some());
    assert!(Rank::from_number(0).is_none());
    assert!(Rank::from_number(9).is_none());
}

/// Example: Converting rank to number
/// Original: Rank::to_number() doc example
#[test]
fn example_rank_to_number() {
    let rank = Rank(3);
    assert_eq!(rank.to_number(), 4);

    // Test boundaries
    assert_eq!(Rank(0).to_number(), 1);
    assert_eq!(Rank(7).to_number(), 8);
}

// Square tests

/// Example: Creating a square from indices
/// Original: Square::new() doc example
#[test]
fn example_square_new() {
    let square = Square::new(4, 3); // e4
    assert_eq!(square.file.index(), 4);
    assert_eq!(square.rank.index(), 3);
}

/// Example: Creating a square from algebraic notation
/// Original: Square::from_algebraic() doc example
#[test]
fn example_square_from_algebraic() {
    let square = Square::from_algebraic("e4").unwrap();
    assert_eq!(square.file.index(), 4);
    assert_eq!(square.rank.index(), 3);

    // Test other squares
    let a1 = Square::from_algebraic("a1").unwrap();
    assert_eq!(a1.file.index(), 0);
    assert_eq!(a1.rank.index(), 0);

    let h8 = Square::from_algebraic("h8").unwrap();
    assert_eq!(h8.file.index(), 7);
    assert_eq!(h8.rank.index(), 7);
}

/// Example: Converting square to algebraic notation
/// Original: Square::to_algebraic() doc example
#[test]
fn example_square_to_algebraic() {
    let square = Square::new(4, 3);
    assert_eq!(square.to_algebraic(), "e4");

    assert_eq!(Square::new(0, 0).to_algebraic(), "a1");
    assert_eq!(Square::new(7, 7).to_algebraic(), "h8");
}

// Centipawns tests

/// Example: Getting piece values in centipawns
#[test]
fn example_centipawns_for_piece() {
    assert_eq!(Centipawns::for_piece(PieceType::Pawn).value(), 100);
    assert_eq!(Centipawns::for_piece(PieceType::Knight).value(), 300);
    assert_eq!(Centipawns::for_piece(PieceType::Bishop).value(), 300);
    assert_eq!(Centipawns::for_piece(PieceType::Rook).value(), 500);
    assert_eq!(Centipawns::for_piece(PieceType::Queen).value(), 900);
    assert_eq!(Centipawns::for_piece(PieceType::King).value(), 0);
}

/// Example: Arithmetic with centipawns
#[test]
fn example_centipawns_arithmetic() {
    let knight_value = Centipawns::for_piece(PieceType::Knight);
    let pawn_value = Centipawns::for_piece(PieceType::Pawn);

    // Knight is worth 3 pawns
    assert_eq!((knight_value.value() - pawn_value.value() * 3), 0);

    // Material calculation
    let total_value = knight_value.value() + pawn_value.value() + pawn_value.value();
    assert_eq!(total_value, 500); // 300 + 100 + 100
}

/// Example: Converting centipawns to pawns
#[test]
fn example_centipawns_to_pawns() {
    let queen_value = Centipawns::for_piece(PieceType::Queen);
    assert_eq!(queen_value.to_pawns(), 9.0);

    let knight_value = Centipawns::for_piece(PieceType::Knight);
    assert_eq!(knight_value.to_pawns(), 3.0);
}
