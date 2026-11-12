use bevy::prelude::*;

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct Selection {
    pub selected_entity: Option<Entity>,
    pub selected_position: Option<(u8, u8)>,
    pub possible_moves: Vec<(u8, u8)>,
    pub is_dragging: bool,
    pub drag_start: Option<(u8, u8)>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.selected_entity = None;
        self.selected_position = None;
        self.possible_moves.clear();
        self.end_drag();
    }

    pub fn is_selected(&self) -> bool {
        self.selected_entity.is_some()
    }

    pub fn begin_drag(&mut self) {
        self.is_dragging = true;
        if self.drag_start.is_none() {
            self.drag_start = self.selected_position;
        }
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_default() {
        let selection = Selection::default();

        assert!(selection.selected_entity.is_none());
        assert!(selection.selected_position.is_none());
        assert!(selection.possible_moves.is_empty());
        assert!(!selection.is_selected());
        assert!(!selection.is_dragging);
        assert!(selection.drag_start.is_none());
    }

    #[test]
    fn test_selection_is_selected_when_entity_set() {
        let selection = Selection {
            selected_entity: Some(Entity::PLACEHOLDER),
            ..Default::default()
        };

        assert!(selection.is_selected());
    }

    #[test]
    fn test_selection_is_not_selected_initially() {
        let selection = Selection::default();
        assert!(!selection.is_selected());
    }

    #[test]
    fn test_selection_clear_removes_entity() {
        let mut selection = Selection {
            selected_entity: Some(Entity::PLACEHOLDER),
            is_dragging: true,
            drag_start: Some((0, 0)),
            ..Default::default()
        };

        selection.clear();

        assert!(selection.selected_entity.is_none());
        assert!(!selection.is_selected());
        assert!(!selection.is_dragging);
        assert!(selection.drag_start.is_none());
    }

    #[test]
    fn test_selection_clear_removes_position() {
        let mut selection = Selection {
            selected_position: Some((3, 4)),
            ..Default::default()
        };

        selection.clear();

        assert!(selection.selected_position.is_none());
    }

    #[test]
    fn test_selection_clear_removes_possible_moves() {
        let mut selection = Selection {
            possible_moves: vec![(1, 2), (3, 4), (5, 6)],
            drag_start: Some((1, 2)),
            is_dragging: true,
            ..Default::default()
        };

        selection.clear();

        assert!(selection.possible_moves.is_empty());
        assert!(!selection.is_dragging);
        assert!(selection.drag_start.is_none());
    }

    #[test]
    fn test_selection_with_position_and_moves() {
        let selection = Selection {
            selected_entity: Some(Entity::PLACEHOLDER),
            selected_position: Some((2, 1)),
            possible_moves: vec![(2, 2), (2, 3)],
            ..Default::default()
        };

        assert!(selection.is_selected());
        assert_eq!(selection.selected_position, Some((2, 1)));
        assert_eq!(selection.possible_moves.len(), 2);
    }

    #[test]
    fn test_selection_possible_moves_can_be_empty() {
        let selection = Selection {
            selected_entity: Some(Entity::PLACEHOLDER),
            selected_position: Some((4, 4)),
            possible_moves: vec![],
            ..Default::default()
        }; // Surrounded piece

        assert!(selection.is_selected());
        assert!(selection.possible_moves.is_empty());
    }

    #[test]
    fn test_selection_multiple_possible_moves() {
        let moves: Vec<(u8, u8)> = (0..8).map(|i| (i, i)).collect();
        let selection = Selection {
            possible_moves: moves.clone(),
            ..Default::default()
        };

        assert_eq!(selection.possible_moves.len(), 8);
        assert_eq!(selection.possible_moves, moves);
    }

    #[test]
    fn test_selection_clear_is_idempotent() {
        let mut selection = Selection {
            selected_entity: Some(Entity::PLACEHOLDER),
            drag_start: Some((2, 2)),
            is_dragging: true,
            ..Default::default()
        };
        selection.clear();
        selection.clear(); // Should not panic or cause issues

        assert!(!selection.is_selected());
        assert!(!selection.is_dragging);
        assert!(selection.drag_start.is_none());
    }

    #[test]
    fn test_selection_begin_and_end_drag() {
        let mut selection = Selection {
            selected_position: Some((3, 3)),
            ..Default::default()
        };
        selection.begin_drag();

        assert!(selection.is_dragging);
        assert_eq!(selection.drag_start, Some((3, 3)));

        selection.end_drag();
        assert!(!selection.is_dragging);
        assert!(selection.drag_start.is_none());
    }
}
