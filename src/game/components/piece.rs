//! Piece-related ECS components for chess game state tracking
//!
//! This module defines the core ECS components that track piece state in XFChess.
//! These components work together with the [`crate::rendering::pieces::Piece`] component
//! to provide complete piece functionality.
//!
//! # Component Architecture
//!
//! - **[`SelectedPiece`]**: Marks a piece as currently selected by the player
//! - **[`HasMoved`]**: Tracks movement history for chess rule enforcement
//!
//! # Design Pattern: Composition over Inheritance
//!
//! Rather than a single monolithic `Piece` struct, we use separate components:
//! - `Piece` (from rendering) - Type & color (immutable)
//! - `HasMoved` - Movement state (mutable)
//! - `SelectedPiece` - UI state (temporary)
//! - `Transform` (Bevy) - Position (mutable)
//!
//! This allows efficient querying. For example:
//! - "All pieces that haven't moved" → `Query<&Piece, Without<HasMoved>>`
//! - "Selected piece info" → `Query<&Piece, With<SelectedPiece>>`
//!
//! # Integration with Chess Rules
//!
//! These components are used by:
//! - [`crate::game::resources::ChessEngine`] - Unified chess engine for move validation
//! - [`crate::game::systems::input`] - Piece selection handling
//! - [`crate::game::systems::visual`] - Highlighting & animations
//!
//! # Reference
//!
//! ECS component patterns based on:
//! - `reference/bevy/examples/ecs/component_change_detection.rs`
//! - `reference/bevy-3d-chess/src/pieces.rs` (comparison implementation)
//!
//! # Examples
//!
//! ## Marking a piece as moved
//!
//! ```rust
//! use bevy::prelude::*;
//! use xfchess::game::components::HasMoved;
//!
//! fn move_piece_system(
//!     mut pieces: Query<&mut HasMoved>,
//!     /* other params */
//! ) {
//!     for mut has_moved in pieces.iter_mut() {
//!         has_moved.moved = true;
//!         has_moved.move_count += 1;
//!     }
//! }
//! ```
//!
//! ## Checking if a piece is selected
//!
//! ```rust
//! use bevy::prelude::*;
//! use xfchess::game::components::SelectedPiece;
//!
//! fn highlight_system(
//!     selected: Query<Entity, With<SelectedPiece>>,
//! ) {
//!     if let Ok(entity) = selected.get_single() {
//!         println!("Piece {:?} is selected", entity);
//!     }
//! }
//! ```

use bevy::prelude::*;

/// Component marking a piece as currently selected by the player
///
/// This component is added to a piece entity when the player clicks on it,
/// enabling systems to:
/// - Highlight the selected piece visually
/// - Show available moves
/// - Prevent simultaneous multi-piece selection
///
/// # Lifecycle
///
/// **Added when:**
/// - Player clicks a piece they can move
/// - AI selects a piece to move (visualizes AI thinking)
///
/// **Removed when:**
/// - The piece completes its move
/// - Player selects a different piece
/// - Player clicks an empty square (deselects)
/// - Game state changes (check/checkmate/end)
///
/// # Fields
///
/// - `entity`: The Bevy entity ID of the selected piece (for quick lookups)
/// - `position`: Board position (x, y) where x,y ∈ [0,7] (for move validation)
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use xfchess::game::components::SelectedPiece;
///
/// fn select_piece(
///     mut commands: Commands,
///     piece_entity: Entity,
///     position: (u8, u8),
/// ) {
///     commands.entity(piece_entity).insert(SelectedPiece {
///         entity: piece_entity,
///         position,
///     });
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Reflect)]
#[reflect(Component)]
pub struct SelectedPiece {
    /// Bevy entity handle for the selected piece
    ///
    /// Stored for quick access without re-querying the ECS when systems
    /// need to operate on the selected piece.
    pub entity: Entity,

    /// Board position (x, y) in array coordinates
    ///
    /// - x: File (0=a, 7=h)
    /// - y: Rank (0=1, 7=8)
    ///
    /// Used for move validation and highlighting legal destination squares.
    pub position: (u8, u8),
}

/// Component tracking whether a piece has moved from its starting position
///
/// Critical for enforcing chess rules that depend on movement history:
/// - **Castling**: King and rook must not have moved
/// - **Pawn double-move**: Pawns can move 2 squares only on first move
/// - **En passant**: Capture available only after opponent's pawn double-move
///
/// # Design Decision
///
/// We track both a boolean flag AND a move count:
/// - `moved`: Fast check for "has this piece ever moved?"
/// - `move_count`: Detailed history for UI/analysis (shows "veteran" pieces)
///
/// # Initialization
///
/// Pieces get this component in three ways:
/// 1. **Default (unmoved)**: `.insert(HasMoved::default())` when spawning
/// 2. **Explicit**: `.insert(HasMoved { moved: true, move_count: 5 })`
/// 3. **Load from saved game**: Restore exact movement state
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use xfchess::game::components::HasMoved;
///
/// // Check if castling is legal (king must not have moved)
/// fn can_castle(king_has_moved: &HasMoved) -> bool {
///     !king_has_moved.moved
/// }
///
/// // Update after a move
/// fn record_move(mut has_moved: Mut<HasMoved>) {
///     has_moved.moved = true;
///     has_moved.move_count += 1;
/// }
/// ```
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct HasMoved {
    /// Whether this piece has ever moved from its starting square
    ///
    /// - `false`: Piece is still on its initial square (enables special moves)
    /// - `true`: Piece has moved at least once (disables castling rights, etc.)
    pub moved: bool,

    /// Total number of times this piece has moved
    ///
    /// Used for:
    /// - Game analysis (identify "active" vs "passive" pieces)
    /// - UI hints ("This rook has moved 3 times")
    /// - Future AI evaluation (piece activity heuristic)
    pub move_count: u32,
}

/// Component marking a piece as captured
///
/// When a piece is captured, it's moved to a capture zone outside the board
/// instead of being despawned. This component marks pieces that are in the
/// capture zone and should not be considered part of the active game board.
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Captured;

/// Component for fading out a captured piece before moving to capture zone
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct FadingCapture {
    /// Timer tracking fade duration
    pub timer: Timer,
    /// Target position in capture zone (applied after fade completes)
    pub capture_zone_pos: Vec3,
}

/// Component representing an active straight-line animation for a piece move
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct PieceMoveAnimation {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

impl PieceMoveAnimation {
    pub fn new(start: Vec3, end: Vec3, duration: f32) -> Self {
        Self {
            start,
            end,
            elapsed: 0.0,
            duration: duration.max(f32::EPSILON),
        }
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selected_piece_creation() {
        //! Verifies SelectedPiece can be created with valid data

        let entity = Entity::from_bits(42);
        let position = (3, 4);

        let selected = SelectedPiece { entity, position };

        assert_eq!(selected.entity, entity);
        assert_eq!(selected.position, (3, 4));
    }

    #[test]
    fn test_selected_piece_clone() {
        //! Tests SelectedPiece implements Clone correctly

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
        //! Verifies SelectedPiece implements Copy (efficient passing)

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
        //! Tests debug output is useful for logging

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
        //! Verifies HasMoved defaults to unmoved state

        let has_moved = HasMoved::default();

        assert_eq!(has_moved.moved, false);
        assert_eq!(has_moved.move_count, 0);
    }

    #[test]
    fn test_has_moved_explicit_creation() {
        //! Tests creating HasMoved with specific values

        let has_moved = HasMoved {
            moved: true,
            move_count: 3,
        };

        assert_eq!(has_moved.moved, true);
        assert_eq!(has_moved.move_count, 3);
    }

    #[test]
    fn test_has_moved_mutation() {
        //! Simulates recording a move

        let mut has_moved = HasMoved::default();

        // Piece hasn't moved yet
        assert!(!has_moved.moved);
        assert_eq!(has_moved.move_count, 0);

        // Record first move
        has_moved.moved = true;
        has_moved.move_count += 1;

        assert!(has_moved.moved);
        assert_eq!(has_moved.move_count, 1);

        // Record second move
        has_moved.move_count += 1;

        assert_eq!(has_moved.move_count, 2);
    }

    #[test]
    fn test_has_moved_castling_check() {
        //! Tests the common "can castle?" pattern

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
        //! Verifies HasMoved can be cloned

        let original = HasMoved {
            moved: true,
            move_count: 5,
        };
        let cloned = original.clone();

        assert_eq!(original.moved, cloned.moved);
        assert_eq!(original.move_count, cloned.move_count);
    }

    #[test]
    fn test_has_moved_copy() {
        //! Tests HasMoved implements Copy semantics

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
        //! Verifies debug output is informative

        let has_moved = HasMoved {
            moved: true,
            move_count: 7,
        };
        let debug_str = format!("{:?}", has_moved);

        assert!(debug_str.contains("HasMoved"));
        assert!(debug_str.contains("true") || debug_str.contains("7"));
    }

    #[test]
    fn test_piece_move_animation_new() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(1.0, 0.0, 1.0);
        let anim = PieceMoveAnimation::new(start, end, 0.3);

        assert_eq!(anim.start, start);
        assert_eq!(anim.end, end);
        assert_eq!(anim.elapsed, 0.0);
        assert!(anim.duration > 0.0);
        assert_eq!(anim.progress(), 0.0);
    }
}
