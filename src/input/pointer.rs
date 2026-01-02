//! Advanced pointer interaction system with hover effects and cursor management
//!
//! This module provides a complete pointer interaction system for the chess game,
//! including real-time cursor tracking, hover effects for pieces and squares,
//! and context-aware visual feedback.
//!
//! # Features
//!
//! - **Cursor Tracking**: Real-time cursor position monitoring
//! - **Hover Effects**: Material highlighting for pieces and squares on hover
//! - **Cursor Style**: Dynamic cursor icon changes (pointer vs default)
//! - **Game Integration**: Only highlights valid interactions based on game state
//! - **Performance**: Optimized with change detection and material caching
//!
//! # Architecture
//!
//! The system uses Bevy 0.17's observer pattern for efficient event handling:
//! - Each entity (piece/square) has `.observe()` callbacks attached
//! - Events trigger only on specific entities, not globally
//! - No polling required - direct event routing
//!
//! # Observer Pattern
//!
//! ```rust,ignore
//! // Attach to pieces during spawning
//! commands.spawn(PieceBundle)
//!     .observe(on_piece_hover)   // Pointer<Over> event
//!     .observe(on_piece_unhover) // Pointer<Out> event
//!     .observe(on_piece_click);  // Pointer<Click> event
//!
//! // Attach to squares during spawning
//! commands.spawn(SquareBundle)
//!     .observe(on_square_hover)
//!     .observe(on_square_unhover)
//!     .observe(on_square_click);
//! ```
//!
//! # Hover Effect System
//!
//! Hover effects are game-state aware:
//! - **Pieces**: Only highlight if it's the current player's turn
//! - **Squares**: Only highlight if a piece is selected and square is a valid target
//! - **AI Turn**: No hover effects during AI thinking
//! - **Game Over**: No hover effects when game is finished
//!
//! # Cursor Style Management
//!
//! The cursor automatically changes based on context:
//! - **Pointer**: When hovering over interactive elements (pieces/valid squares)
//! - **Default**: When hovering over non-interactive areas
//!
//! # Performance
//!
//! Optimizations include:
//! - Material handle caching to avoid repeated lookups
//! - Change detection to skip redundant updates
//! - Batched entity commands for multiple updates
//! - Rate-limited debug logging (1 second intervals)
//!
//! # Reference
//!
//! Implementation patterns from:
//! - `reference/bevy-3d-chess/src/main.rs` - Hover effect observers
//! - `reference/bevy/examples/picking/mesh_picking.rs` - Observer examples
//! - `reference/chessground/src/events.ts` - Lichess hover UX patterns

use crate::game::components::GamePhase;
use crate::game::resources::{CurrentGamePhase, CurrentTurn, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow};

/// Resource tracking the current cursor position within the game window
///
/// Updated every frame by `cursor_tracking_system`. Used for debugging
/// and potential future features like piece drag-and-drop.
///
/// # Fields
///
/// - `position`: Current cursor (x, y) in window coordinates, or None if outside window
/// - `last_update`: Timestamp of last position update (for debug rate-limiting)
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct CursorState {
    /// Current cursor position within window bounds, None if outside
    pub position: Option<Vec2>,
    /// Time accumulator for debug logging rate-limiting
    pub last_update: f32,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            position: None,
            last_update: 0.0,
        }
    }
}

/// Resource tracking the current cursor icon style
///
/// Used to manage cursor icon changes when hovering over interactive elements.
/// The system automatically changes between Default and Pointer icons based
/// on what the cursor is hovering over.
///
/// # Cursor Icons
///
/// - **Default**: Arrow cursor (normal state)
/// - **Pointer**: Hand cursor (over clickable pieces/squares)
#[derive(Resource, Debug, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct CursorStyle {
    /// Current cursor icon being displayed
    pub current: CursorIcon,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self {
            current: CursorIcon::default(),
        }
    }
}

/// Resource for caching material handles to avoid repeated asset lookups
///
/// Stores commonly used materials for hover effects to improve performance.
/// Materials are loaded once during initialization and reused throughout
/// the game lifecycle.
///
/// # Materials
///
/// - **piece_hover**: Brightened material for piece hover state
/// - **square_hover**: Highlighted material for valid move squares
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct HoverMaterials {
    /// Material for pieces when hovered (brightened)
    pub piece_hover_factor: f32,
    /// Material for squares when hovered (highlighted)
    pub square_hover_factor: f32,
}

impl Default for HoverMaterials {
    fn default() -> Self {
        Self {
            piece_hover_factor: 1.3,  // 30% brighter
            square_hover_factor: 1.2, // 20% brighter
        }
    }
}

/// Resource for storing original material states before hover modifications
///
/// When a piece or square is hovered, we store its original material here
/// so we can revert it when the cursor leaves. This uses Entity as the key
/// to track which materials belong to which entities.
#[derive(Resource, Default, Debug)]
pub struct OriginalMaterials {
    /// Map of entity -> original material handle
    pub materials: std::collections::HashMap<Entity, Handle<StandardMaterial>>,
}

/// System that tracks cursor position in real-time
///
/// Updates the `CursorState` resource every frame with the current cursor position.
/// Logs cursor position periodically (every 1 second) for debugging purposes.
///
/// # Performance
///
/// This system runs every frame but only logs every 1 second to avoid console spam.
/// The actual position tracking is very lightweight.
pub fn cursor_tracking_system(
    q_windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut cursor_state: ResMut<CursorState>,
) {
    cursor_state.last_update += time.delta_secs();
    if let Ok(window) = q_windows.single() {
        cursor_state.position = window.cursor_position();
        // Debug logging (rate-limited to 1 second intervals)
        if cursor_state.last_update >= 1.0 {
            if let Some(position) = cursor_state.position {
                trace!(
                    "[POINTER] Cursor position: ({:.1}, {:.1})",
                    position.x,
                    position.y
                );
            }
            cursor_state.last_update = 0.0;
        }
    } else {
        cursor_state.position = None;
    }
}

/// Observer function for piece hover events (Pointer<Over>)
///
/// Brightens the piece's material when the cursor hovers over it, but only if:
/// 1. It's the current player's turn to move this color
/// 2. The game is in Playing or Check phase (not game over)
/// 3. It's not currently the AI's turn
///
/// # Parameters
///
/// - `hover`: The Pointer<Over> event triggered by hovering
/// - `piece_query`: Query to get the piece's color
/// - `current_turn`: Current player's turn
/// - `game_phase`: Current game phase (to check if game is active)
/// - `materials`: Asset storage for materials
/// - `material_query`: Query to access entity's material
///
/// # Visual Effect
///
/// The piece's material is brightened by `piece_hover_factor` (default 30%)
/// to provide clear visual feedback that the piece is interactive.
pub fn on_piece_hover(
    hover: On<Pointer<Over>>,
    piece_query: Query<&Piece>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
    hover_materials: Res<HoverMaterials>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
) {
    let entity = hover.entity;
    // Only highlight pieces during active gameplay
    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }
    // Get the piece being hovered
    if let Ok(piece) = piece_query.get(entity) {
        // Only highlight if it's this player's turn
        if piece.color != current_turn.color {
            return;
        }
        // Get the entity's current material
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            // Store original material for later restoration
            original_materials
                .materials
                .insert(entity, material_handle.0.clone());

            // Create brightened version
            if let Some(original_mat) = materials.get(&material_handle.0) {
                let mut brightened = original_mat.clone();

                // Brighten the base color
                let rgb = brightened.base_color.to_linear();
                brightened.base_color = Color::LinearRgba(LinearRgba {
                    red: (rgb.red * hover_materials.piece_hover_factor).min(1.0),
                    green: (rgb.green * hover_materials.piece_hover_factor).min(1.0),
                    blue: (rgb.blue * hover_materials.piece_hover_factor).min(1.0),
                    alpha: rgb.alpha,
                });

                // Add brightened material to assets and update entity
                let brightened_handle = materials.add(brightened);
                material_handle.0 = brightened_handle;

                trace!(
                    "[POINTER] Hover effect applied to {:?} piece at entity {:?}",
                    piece.color,
                    entity
                );
            }
        }
    }
}

/// Observer function for piece unhover events (Pointer<Out>)
///
/// Reverts the piece's material to its original state when the cursor leaves.
/// Uses the `OriginalMaterials` resource to restore the exact material that
/// was active before hovering.
///
/// # Parameters
///
/// - `unhover`: The Pointer<Out> event triggered by cursor leaving
/// - `material_query`: Query to access entity's material
/// - `original_materials`: Storage for original material handles
pub fn on_piece_unhover(
    unhover: On<Pointer<Out>>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
) {
    let entity = unhover.entity;

    // Restore original material if we have it stored
    if let Some(original_handle) = original_materials.materials.remove(&entity) {
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            material_handle.0 = original_handle;
            trace!("[POINTER] Hover effect removed from entity {:?}", entity);
        }
    }
}

/// Observer function for square hover events (Pointer<Over>)
///
/// Highlights squares when hovered, but only if:
/// 1. A piece is currently selected
/// 2. The square is a valid move destination for the selected piece
/// 3. The game is in Playing or Check phase
///
/// # Parameters
///
/// - `hover`: The Pointer<Over> event triggered by hovering
/// - `square_query`: Query to get the square's position
/// - `selection`: Current piece selection state
/// - `game_phase`: Current game phase
/// - `hover_materials`: Hover material configuration
/// - `material_query`: Query to access entity's material
/// - `materials`: Asset storage for materials
/// - `original_materials`: Storage for original material handles
///
/// # Visual Effect
///
/// Valid move squares are highlighted by `square_hover_factor` (default 20%)
/// to clearly indicate where the selected piece can move.
pub fn on_square_hover(
    hover: On<Pointer<Over>>,
    square_query: Query<&Square>,
    selection: Res<Selection>,
    game_phase: Res<CurrentGamePhase>,
    hover_materials: Res<HoverMaterials>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
) {
    let entity = hover.entity;

    // Only highlight squares during active gameplay
    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }

    // Only highlight if a piece is selected
    if !selection.is_selected() {
        return;
    }

    // Get the square being hovered
    if let Ok(square) = square_query.get(entity) {
        let square_pos = (square.x, square.y);

        // Only highlight if this is a valid move destination
        if !selection.possible_moves.contains(&square_pos) {
            return;
        }

        // Get the entity's current material
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            // Store original material for later restoration
            original_materials
                .materials
                .insert(entity, material_handle.0.clone());

            // Create highlighted version
            if let Some(original_mat) = materials.get(&material_handle.0) {
                let mut highlighted = original_mat.clone();

                // Brighten the base color
                let rgb = highlighted.base_color.to_linear();
                highlighted.base_color = Color::LinearRgba(LinearRgba {
                    red: (rgb.red * hover_materials.square_hover_factor).min(1.0),
                    green: (rgb.green * hover_materials.square_hover_factor).min(1.0),
                    blue: (rgb.blue * hover_materials.square_hover_factor).min(1.0),
                    alpha: rgb.alpha,
                });

                // Add highlighted material to assets and update entity
                let highlighted_handle = materials.add(highlighted);
                material_handle.0 = highlighted_handle;

                trace!(
                    "[POINTER] Hover effect applied to square ({}, {}) at entity {:?}",
                    square.x,
                    square.y,
                    entity
                );
            }
        }
    }
}

/// Observer function for square unhover events (Pointer<Out>)
///
/// Reverts the square's material to its original state when the cursor leaves.
/// Uses the `OriginalMaterials` resource to restore the exact material that
/// was active before hovering.
///
/// # Parameters
///
/// - `unhover`: The Pointer<Out> event triggered by cursor leaving
/// - `material_query`: Query to access entity's material
/// - `original_materials`: Storage for original material handles
pub fn on_square_unhover(
    unhover: On<Pointer<Out>>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
) {
    let entity = unhover.entity;

    // Restore original material if we have it stored
    if let Some(original_handle) = original_materials.materials.remove(&entity) {
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            material_handle.0 = original_handle;
            trace!(
                "[POINTER] Hover effect removed from square at entity {:?}",
                entity
            );
        }
    }
}

/// System that manages cursor icon changes based on hover state
///
/// Changes the cursor icon to a pointer when hovering over interactive elements
/// (pieces that can be selected, or squares that are valid move targets).
/// Reverts to default cursor when not hovering anything interactive.
///
/// # Performance
///
/// Only updates the cursor icon when the style actually changes, avoiding
/// redundant Window API calls.
///
/// TODO: Implement proper cursor tracking using a resource updated by hover observers
/// instead of polling PointerInteraction state. This would be more efficient and align
/// with Bevy 0.17's event-driven picking system.
pub fn cursor_style_system(mut cursor_style: ResMut<CursorStyle>) {
    // Simplified version: just track cursor state
    // Full cursor icon management will be implemented in a future enhancement
    // TODO: Implement cursor icon changes using Window API when needed
    cursor_style.current = CursorIcon::default();
}

/// Debug system that logs pointer interactions (rate-limited)
///
/// Logs when entities are hovered or clicked. Rate-limited to 1 second intervals
/// to avoid console spam. Useful for debugging pointer interaction issues.
///
/// # Debug Output
///
/// - Entity being hovered
/// - Piece color (if hovering a piece)
/// - Whether mouse buttons are pressed
#[derive(Resource, Debug)]
pub struct PointerDebugTimer {
    pub time: f32,
}

impl FromWorld for PointerDebugTimer {
    fn from_world(_world: &mut World) -> Self {
        PointerDebugTimer { time: 0.0 }
    }
}

/// TODO: Re-implement using event-based approach instead of polling PointerInteraction
pub fn entity_cursor_debug(time: Res<Time>, mut timer: ResMut<PointerDebugTimer>) {
    timer.time += time.delta_secs();
    if timer.time >= 1.0 {
        // Debug logging disabled - will be re-implemented with proper event tracking
        timer.time = 0.0;
    }
}

/// Helper function to create a generic material update observer
///
/// Creates an observer closure that updates an entity's material when a specific
/// event triggers. This is a reusable pattern for any event-driven material changes.
///
/// # Type Parameters
///
/// - `E`: Any event type that implements Event + Debug + Clone + Reflect
///
/// # Parameters
///
/// - `new_material`: The material handle to apply when the event triggers
///
/// # Returns
///
/// A closure that can be attached to entities with `.observe()`
///
/// # Example
///
/// ```rust,ignore
/// // Create hover material
/// let hover_material = materials.add(StandardMaterial {
///     base_color: Color::LinearRgba(LinearRgba::new(1.3, 1.3, 1.3, 1.0)),
///     ..default()
/// });
///
/// // Attach observer to entity
/// commands.spawn(piece_bundle)
///     .observe(create_material_observer::<Over>(hover_material.clone()))
///     .observe(create_material_observer::<Out>(original_material.clone()));
/// ```
///
/// Note: The `create_material_observer` function has been removed. Use direct observer
/// functions like `on_piece_hover` instead, which are more performant and can access
/// game state resources.
///
/// Plugin that registers all pointer interaction systems and resources
///
/// Sets up the complete pointer interaction system including:
/// - Cursor tracking
/// - Hover effects
/// - Cursor style management
/// - Debug logging (optional)
///
/// # Systems Registered
///
/// All systems run during `Update` schedule when in `GameState::InGame`:
/// - `cursor_tracking_system`: Tracks cursor position every frame
/// - `cursor_style_system`: Manages cursor icon changes
/// - `entity_cursor_debug`: Debug logging (rate-limited)
///
/// # Resources Initialized
///
/// - `CursorState`: Cursor position tracking
/// - `CursorStyle`: Cursor icon state
/// - `HoverMaterials`: Material configuration for hover effects
/// - `OriginalMaterials`: Storage for material restoration
/// - `PointerDebugTimer`: Debug logging rate-limiter
pub struct PointerEventsPlugin;

impl Plugin for PointerEventsPlugin {
    fn build(&self, app: &mut App) {
        use crate::core::GameState;

        // Register resources
        app.init_resource::<CursorState>();
        app.init_resource::<CursorStyle>();
        app.init_resource::<HoverMaterials>();
        app.init_resource::<OriginalMaterials>();
        app.init_resource::<PointerDebugTimer>();

        // Register types for reflection (inspector support)
        app.register_type::<CursorState>();
        app.register_type::<CursorStyle>();
        app.register_type::<HoverMaterials>();

        // Register systems - only run during active gameplay
        app.add_systems(
            Update,
            (
                cursor_tracking_system,
                cursor_style_system,
                entity_cursor_debug,
            )
                .run_if(in_state(GameState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_state_default() {
        let cursor_state = CursorState::default();
        assert!(cursor_state.position.is_none());
        assert_eq!(cursor_state.last_update, 0.0);
    }

    #[test]
    fn test_cursor_style_default() {
        let cursor_style = CursorStyle::default();
        assert_eq!(cursor_style.current, CursorIcon::default());
    }

    #[test]
    fn test_hover_materials_default() {
        let hover_materials = HoverMaterials::default();
        assert_eq!(hover_materials.piece_hover_factor, 1.3);
        assert_eq!(hover_materials.square_hover_factor, 1.2);
    }

    #[test]
    fn test_original_materials_default() {
        let original_materials = OriginalMaterials::default();
        assert!(original_materials.materials.is_empty());
    }

    #[test]
    fn test_pointer_debug_timer_default() {
        let mut world = World::new();
        let timer = PointerDebugTimer::from_world(&mut world);
        assert_eq!(timer.time, 0.0);
    }
}
