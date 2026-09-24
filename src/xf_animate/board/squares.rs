use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::coordinates::{square_world, SQUARE_SIZE};
use crate::core::{DespawnOnExit, GameState};
use crate::xf_animate::viewport::MINI_LAYER;

#[derive(Component)]
pub struct MiniSquare;

pub fn spawn_mini_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(SQUARE_SIZE, 0.05, SQUARE_SIZE));

    let light_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.78, 0.72),
        perceptual_roughness: 0.92,
        ..default()
    });
    let dark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.44, 0.24),
        perceptual_roughness: 0.92,
        ..default()
    });

    for file in 0..8u8 {
        for rank in 0..8u8 {
            let is_light = (file + rank) % 2 == 1;
            let mat = if is_light {
                light_mat.clone()
            } else {
                dark_mat.clone()
            };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(square_world(file, rank)),
                MiniSquare,
                RenderLayers::layer(MINI_LAYER),
                DespawnOnExit(GameState::MainMenu),
                Name::new(format!("MiniSquare {}{}", (b'a' + file) as char, rank + 1)),
            ));
        }
    }
}
