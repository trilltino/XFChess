use crate::engine::board_state::ChessEngine;
use crate::game::components::GamePhase;
use crate::game::resources::CurrentGamePhase;
use crate::rendering::pieces::{Piece, PieceType};
use bevy::prelude::*;

#[derive(Component)]
pub struct CheckHighlightLight;

pub fn update_check_highlight_system(
    mut commands: Commands,
    game_phase: Res<CurrentGamePhase>,
    engine: Res<ChessEngine>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    pieces: Query<&Piece>,
    existing: Query<Entity, With<CheckHighlightLight>>,
    time: Res<Time>,
    mut lights: Query<(&mut PointLight, &mut Transform), With<CheckHighlightLight>>,
) {
    if *view_mode == crate::game::view_mode::ViewMode::Standard2D {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let in_check = matches!(game_phase.0, GamePhase::Check | GamePhase::Checkmate);

    // If not in check, despawn any existing highlight
    if !in_check {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Read the checked side from the engine; replay or network lag can leave the display turn stale.
    let king_color = engine.side_to_move();
    let king_pos = pieces
        .iter()
        .find(|p| p.piece_type == PieceType::King && p.color == king_color)
        .map(|p| Vec3::new(7.0 - p.x as f32, 1.2, p.y as f32));

    let Some(pos) = king_pos else { return };

    if existing.is_empty() {
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
            bevy::camera::visibility::RenderLayers::layer(
                crate::game::systems::camera::BOARD_LAYER,
            ),
        ));
    } else {
        let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5;
        let intensity = 8_000.0 + pulse * 24_000.0;
        for (mut light, mut tf) in lights.iter_mut() {
            light.intensity = intensity;
            tf.translation = pos;
        }
    }
}
