//! Move hints visualization system
//!
//! Highlights valid move squares when a piece is selected and show_hints is enabled.
//! Regular moves: green dot. Captures: orange annular ring (Lichess style).

use crate::core::GameSettings;
use crate::game::components::Piece;
use crate::game::resources::Selection;
use crate::rendering::utils::SquareMaterials;
use bevy::prelude::*;

/// Marker component for squares showing move hints
#[derive(Component)]
pub struct MoveHint;

/// System that shows/hides move hints based on selection and settings.
/// Uses green dots for regular moves and orange rings for capture targets.
pub fn update_move_hints_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    selection: Res<Selection>,
    hint_query: Query<Entity, With<MoveHint>>,
    materials: Res<SquareMaterials>,
    pieces: Query<&Piece>,
) {
    if !selection.is_changed() && !settings.is_changed() {
        return;
    }

    for entity in hint_query.iter() {
        commands.entity(entity).despawn();
    }

    if settings.show_hints && selection.is_selected() {
        // Build a set of occupied squares for O(1) capture detection
        let occupied: std::collections::HashSet<(u8, u8)> = pieces
            .iter()
            .map(|p| (p.x, p.y))
            .collect();

        for &(x, y) in &selection.possible_moves {
            let is_capture = occupied.contains(&(x, y));
            let (mesh, matl) = if is_capture {
                (materials.capture_hint_mesh.clone(), materials.capture_hint_matl.clone())
            } else {
                (materials.hint_mesh.clone(), materials.hover_matl.clone())
            };
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(matl),
                Transform::from_translation(Vec3::new(7.0 - x as f32, 0.051, y as f32))
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                MoveHint,
                bevy::picking::Pickable::IGNORE,
                Name::new(if is_capture { "Capture Hint" } else { "Move Hint" }),
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }
    }
}
