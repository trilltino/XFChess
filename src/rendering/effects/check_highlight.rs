//! Pulsing red point light on the king square when in check.

use crate::game::components::GamePhase;
use crate::game::resources::CurrentGamePhase;
use crate::game::resources::CurrentTurn;
use crate::rendering::pieces::{Piece, PieceType};
use bevy::prelude::*;

/// Marker component for the check highlight point light entity.
#[derive(Component)]
pub struct CheckHighlightLight;

/// System that spawns or despawns a pulsing red PointLight on the king in check.
pub fn update_check_highlight_system(
    mut commands: Commands,
    game_phase: Res<CurrentGamePhase>,
    current_turn: Res<CurrentTurn>,
    pieces: Query<&Piece>,
    existing: Query<Entity, With<CheckHighlightLight>>,
    time: Res<Time>,
    mut lights: Query<(&mut PointLight, &mut Transform), With<CheckHighlightLight>>,
) {
    let in_check = matches!(game_phase.0, GamePhase::Check | GamePhase::Checkmate);

    // If not in check, despawn any existing highlight
    if !in_check {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Find king position (the side that is in check = current turn's king)
    let king_color = current_turn.color;
    let king_pos = pieces
        .iter()
        .find(|p| p.piece_type == PieceType::King && p.color == king_color)
        .map(|p| Vec3::new(7.0 - p.x as f32, 1.2, p.y as f32));

    let Some(pos) = king_pos else { return };

    if existing.is_empty() {
        // Spawn the light
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.1, 0.1),
                intensity: 20_000.0,
                radius: 1.5,
                range: 3.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(pos),
            CheckHighlightLight,
            Name::new("Check Highlight Light"),
            crate::core::DespawnOnExit(crate::core::GameState::InGame),
        ));
    } else {
        // Update position and pulse intensity
        let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5;
        let intensity = 8_000.0 + pulse * 24_000.0;
        for (mut light, mut tf) in lights.iter_mut() {
            light.intensity = intensity;
            tf.translation = pos;
        }
    }
}
