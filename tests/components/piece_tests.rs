//! Integration tests for piece components extracted from doc tests
//!
//! These tests demonstrate usage patterns for piece-related components.
//! Original location: src/game/components/piece.rs module docs

use bevy::prelude::*;
use xfchess::game::components::{HasMoved, SelectedPiece};

/// Test demonstrating how to mark a piece as moved
/// Original: piece.rs module-level example
#[test]
fn example_move_piece_system() {
    // This demonstrates the pattern for updating HasMoved in a system
    let mut has_moved = HasMoved::default();

    // Piece hasn't moved yet
    assert!(!has_moved.moved);
    assert_eq!(has_moved.move_count, 0);

    // Record the move
    has_moved.moved = true;
    has_moved.move_count += 1;

    assert!(has_moved.moved);
    assert_eq!(has_moved.move_count, 1);
}

/// Test demonstrating how to check if a piece is selected
/// Original: piece.rs module-level example
#[test]
fn example_highlight_selected_piece() {
    // Create a test entity
    let entity = Entity::from_bits(42);

    // Simulate selecting a piece
    let selected = SelectedPiece {
        entity,
        position: (4, 4),
    };

    // In a real system, you would query for entities with SelectedPiece
    assert_eq!(selected.entity, entity);
    assert_eq!(selected.position, (4, 4));
}

/// Test demonstrating how to select a piece
/// Original: SelectedPiece struct-level example
#[test]
fn example_select_piece_command() {
    let piece_entity = Entity::from_bits(100);
    let position = (3, 4);

    let selected = SelectedPiece {
        entity: piece_entity,
        position,
    };

    // In real code, you would use commands.entity(piece_entity).insert(selected)
    assert_eq!(selected.entity, piece_entity);
    assert_eq!(selected.position, position);
}

/// Test demonstrating castling check pattern
/// Original: HasMoved struct-level example
#[test]
fn example_can_castle_check() {
    let unmoved_king = HasMoved::default();
    let moved_king = HasMoved {
        moved: true,
        move_count: 1,
    };

    // Castling requires the king hasn't moved
    fn can_castle(king_has_moved: &HasMoved) -> bool {
        !king_has_moved.moved
    }

    assert!(can_castle(&unmoved_king));
    assert!(!can_castle(&moved_king));
}

/// Test demonstrating move recording pattern
/// Original: HasMoved struct-level example
#[test]
fn example_record_move() {
    let mut has_moved = HasMoved::default();

    // Record a move
    has_moved.moved = true;
    has_moved.move_count += 1;

    assert!(has_moved.moved);
    assert_eq!(has_moved.move_count, 1);

    // Record another move
    has_moved.move_count += 1;
    assert_eq!(has_moved.move_count, 2);
}
