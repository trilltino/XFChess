//! Visual update systems for highlighting and animation
//!
//! These systems handle visual feedback for player interactions:
//! - Square highlighting for selected pieces and valid moves
//! - Piece movement animations
//! - Visual state restoration
//!
//! # Execution Order
//!
//! These systems run in the `Visual` system set, after all game logic
//! has been processed. This ensures visual updates reflect the current
//! game state.

use crate::game::components::PieceMoveAnimation;
use crate::game::resources::{CurrentTurn, GameTimer, PendingTurnAdvance, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::{ReturnMaterials, Square, SquareMaterials};
use bevy::prelude::*;

/// System to visually highlight possible moves and selected square
///
/// Updates square materials to provide visual feedback for:
/// - **Selected piece**: Highlights the source square
/// - **Valid moves**: Highlights all legal destination squares
/// - **Restoration**: Restores original colors for unselected squares
///
/// # Execution Order
///
/// Runs in `GameSystems::Visual` set, after all game logic systems.
/// This ensures highlights reflect the current selection state.
///
/// # Performance
///
/// Iterates over all squares each frame. Consider using change detection
/// or event-based updates if this becomes a bottleneck.
pub fn highlight_possible_moves(
    selection: Res<Selection>,
    square_materials: Res<SquareMaterials>,
    return_materials: Res<ReturnMaterials>,
    mut squares_query: Query<(Entity, &Square, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (_, square, mut material) in squares_query.iter_mut() {
        let pos = (square.x, square.y);

        // Check if this square should be highlighted
        let should_highlight = selection.is_selected()
            && (
                selection.selected_position == Some(pos) || // Selected square
            selection.possible_moves.contains(&pos)
                // Valid move destination
            );

        if should_highlight {
            // Handle is Clone (not Copy), need .clone() from Res
            material.0 = square_materials.hover_matl.clone();
        } else {
            // Restore original color
            material.0 = return_materials.get_original_material(square, &square_materials);
        }
    }
}

/// System to animate piece movement
///
/// Smoothly interpolates piece positions from their current transform
/// to their target position based on the Piece component.
///
/// # Execution Order
///
/// Runs in `GameSystems::Visual` set, after game logic updates piece
/// positions but before rendering.
///
/// # Animation Behavior
///
/// - Uses linear interpolation with configurable speed
/// - Snaps to final position when within 0.1 units
/// - Handles both movement and capture animations
pub fn animate_piece_movement(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut Transform,
        &Piece,
        Option<&mut PieceMoveAnimation>,
    )>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut current_turn: ResMut<CurrentTurn>,
    mut game_timer: ResMut<GameTimer>,
) {
    let mut animation_active = false;
    let mut completed = Vec::new();

    for (entity, mut transform, piece, animation) in query.iter_mut() {
        if let Some(mut animation) = animation {
            animation.elapsed = (animation.elapsed + time.delta_secs()).min(animation.duration);
            let progress = animation.progress();
            transform.translation = animation.start.lerp(animation.end, progress);

            if progress >= 1.0 {
                transform.translation = animation.end;
                completed.push(entity);
            } else {
                animation_active = true;
            }
        } else {
            // Rate-limited warning to avoid spam
            let target = Vec3::new(piece.x as f32, 0.0, piece.y as f32);
            if (transform.translation - target).length() > 0.01 {
                transform.translation = target;
            }
        }
    }

    for entity in completed {
        commands.entity(entity).remove::<PieceMoveAnimation>();
    }

    if !animation_active && pending_turn.is_pending() {
        if let Some(pending) = pending_turn.take() {
            let mover = pending.mover;
            game_timer.apply_increment(mover);
            current_turn.switch();

            // Consolidated log: one line instead of three
            debug!(
                "[MOVE] {:?} → {:?} | Move #{} | Times: W={:.1}s B={:.1}s",
                mover,
                current_turn.color,
                current_turn.move_number,
                game_timer.white_time_left,
                game_timer.black_time_left
            );
        }
    }
}
