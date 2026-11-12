use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug, Reflect)]
#[reflect(Component)]
pub struct SelectedPiece {
    pub entity: Entity,

    pub position: (u8, u8),
}

#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct HasMoved {
    pub moved: bool,

    pub move_count: u32,
}

#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Captured;

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct FadingCapture {
    pub timer: Timer,
    pub initial_pos: Vec3,
    pub knockback_dir: Vec3,
    pub tilt_axis: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selected_piece_creation() {
        let entity = Entity::from_bits(42);
        let position = (3, 4);

        let selected = SelectedPiece { entity, position };

        assert_eq!(selected.entity, entity);
        assert_eq!(selected.position, (3, 4));
    }

    #[test]
    fn test_selected_piece_clone() {
        let original = SelectedPiece {
            entity: Entity::from_bits(10),
            position: (2, 2),
        };

        let cloned = original;

        assert_eq!(original.entity, cloned.entity);
        assert_eq!(original.position, cloned.position);
    }

    #[test]
    fn test_selected_piece_copy() {
        let original = SelectedPiece {
            entity: Entity::from_bits(5),
            position: (0, 0),
        };

        let copied = original; // Copy, not move

        // Original still accessible (Copy trait)
        assert_eq!(original.entity, copied.entity);
    }

    #[test]
    fn test_selected_piece_debug_format() {
        let selected = SelectedPiece {
            entity: Entity::from_bits(7),
            position: (4, 4),
        };

        let debug_str = format!("{:?}", selected);

        assert!(debug_str.contains("SelectedPiece"));
        assert!(debug_str.contains("4")); // Position should appear
    }

    #[test]
    fn test_has_moved_default() {
        let has_moved = HasMoved::default();

        assert!(!has_moved.moved);
        assert_eq!(has_moved.move_count, 0);
    }

    #[test]
    fn test_has_moved_explicit_creation() {
        let has_moved = HasMoved {
            moved: true,
            move_count: 3,
        };

        assert!(has_moved.moved);
        assert_eq!(has_moved.move_count, 3);
    }

    #[test]
    fn test_has_moved_mutation() {
        let mut has_moved = HasMoved::default();

        // Piece hasn't moved yet
        assert!(!has_moved.moved);
        assert_eq!(has_moved.move_count, 0);

        has_moved.moved = true;
        has_moved.move_count += 1;

        assert!(has_moved.moved);
        assert_eq!(has_moved.move_count, 1);

        has_moved.move_count += 1;

        assert_eq!(has_moved.move_count, 2);
    }

    #[test]
    fn test_has_moved_castling_check() {
        let unmoved_king = HasMoved::default();
        let moved_king = HasMoved {
            moved: true,
            move_count: 1,
        };

        // Castling requires king hasn't moved
        assert!(!unmoved_king.moved); // Can castle
        assert!(moved_king.moved); // Cannot castle
    }

    #[test]
    fn test_has_moved_clone() {
        let original = HasMoved {
            moved: true,
            move_count: 5,
        };
        let cloned = original;

        assert_eq!(original.moved, cloned.moved);
        assert_eq!(original.move_count, cloned.move_count);
    }

    #[test]
    fn test_has_moved_copy() {
        let original = HasMoved {
            moved: false,
            move_count: 0,
        };
        let copied = original; // Copy, not move

        // Original still accessible
        assert_eq!(original.moved, copied.moved);
    }

    #[test]
    fn test_has_moved_debug_format() {
        let has_moved = HasMoved {
            moved: true,
            move_count: 7,
        };
        let debug_str = format!("{:?}", has_moved);

        assert!(debug_str.contains("HasMoved"));
        assert!(debug_str.contains("true") || debug_str.contains("7"));
    }
}
