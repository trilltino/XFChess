//! Selection resource for tracking selected pieces
//!
//! Manages the currently selected chess piece and its possible moves. This resource
//! is central to the user interaction flow:
//!
//! 1. User clicks a piece -> selection stores the entity and position
//! 2. Game rules calculate valid moves -> stored in possible_moves
//! 3. User clicks destination -> move is executed if valid
//! 4. Selection is cleared -> ready for next interaction
//!
//! # Bevy 0.17 Integration
//!
//! Uses standard `Resource` pattern for global state. Systems query this resource
//! to determine UI state (highlight selected piece) and validate move attempts.
//!
//! # Reference
//!
//! This pattern is common in turn-based games and follows Bevy's resource-based
//! state management approach from `reference/bevy/examples/ecs/resources.rs`.

use bevy::prelude::*;

/// Resource to store currently selected piece
#[derive(Resource, Debug, Default)]
pub struct Selection {
    pub selected_entity: Option<Entity>,
    pub selected_position: Option<(u8, u8)>,
    pub possible_moves: Vec<(u8, u8)>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.selected_entity = None;
        self.selected_position = None;
        self.possible_moves.clear();
    }

    pub fn is_selected(&self) -> bool {
        self.selected_entity.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_default() {
        //! Verifies Selection defaults to empty/unselected state
        let selection = Selection::default();

        assert!(selection.selected_entity.is_none());
        assert!(selection.selected_position.is_none());
        assert!(selection.possible_moves.is_empty());
        assert!(!selection.is_selected());
    }

    #[test]
    fn test_selection_is_selected_when_entity_set() {
        //! Tests is_selected returns true when an entity is selected
        let mut selection = Selection::default();
        selection.selected_entity = Some(Entity::PLACEHOLDER);

        assert!(selection.is_selected());
    }

    #[test]
    fn test_selection_is_not_selected_initially() {
        //! Tests is_selected returns false for default selection
        let selection = Selection::default();
        assert!(!selection.is_selected());
    }

    #[test]
    fn test_selection_clear_removes_entity() {
        //! Verifies clear() removes selected entity
        let mut selection = Selection::default();
        selection.selected_entity = Some(Entity::PLACEHOLDER);

        selection.clear();

        assert!(selection.selected_entity.is_none());
        assert!(!selection.is_selected());
    }

    #[test]
    fn test_selection_clear_removes_position() {
        //! Verifies clear() removes selected position
        let mut selection = Selection::default();
        selection.selected_position = Some((3, 4));

        selection.clear();

        assert!(selection.selected_position.is_none());
    }

    #[test]
    fn test_selection_clear_removes_possible_moves() {
        //! Verifies clear() empties possible moves vector
        let mut selection = Selection::default();
        selection.possible_moves = vec![(1, 2), (3, 4), (5, 6)];

        selection.clear();

        assert!(selection.possible_moves.is_empty());
    }

    #[test]
    fn test_selection_with_position_and_moves() {
        //! Tests a fully populated selection state
        let mut selection = Selection::default();

        selection.selected_entity = Some(Entity::PLACEHOLDER);
        selection.selected_position = Some((2, 1));
        selection.possible_moves = vec![(2, 2), (2, 3)];

        assert!(selection.is_selected());
        assert_eq!(selection.selected_position, Some((2, 1)));
        assert_eq!(selection.possible_moves.len(), 2);
    }

    #[test]
    fn test_selection_possible_moves_can_be_empty() {
        //! Tests that a piece can be selected with zero legal moves
        //!
        //! This can occur when a piece is completely surrounded or in checkmate.
        let mut selection = Selection::default();

        selection.selected_entity = Some(Entity::PLACEHOLDER);
        selection.selected_position = Some((4, 4));
        selection.possible_moves = vec![]; // Surrounded piece

        assert!(selection.is_selected());
        assert!(selection.possible_moves.is_empty());
    }

    #[test]
    fn test_selection_multiple_possible_moves() {
        //! Tests selection can store many possible moves (e.g., queen or rook)
        let mut selection = Selection::default();

        let moves: Vec<(u8, u8)> = (0..8).map(|i| (i, i)).collect();
        selection.possible_moves = moves.clone();

        assert_eq!(selection.possible_moves.len(), 8);
        assert_eq!(selection.possible_moves, moves);
    }

    #[test]
    fn test_selection_clear_is_idempotent() {
        //! Verifies calling clear() multiple times is safe
        let mut selection = Selection::default();

        selection.selected_entity = Some(Entity::PLACEHOLDER);
        selection.clear();
        selection.clear(); // Should not panic or cause issues

        assert!(!selection.is_selected());
    }
}
