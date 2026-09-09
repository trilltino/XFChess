use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::viewport::MINI_LAYER;
use crate::core::{DespawnOnExit, GameState};

pub const SQUARE_SIZE: f32 = 1.0;
pub const BOARD_HALF: f32 = 4.0;

#[derive(Component)]
pub struct MiniSquare;

pub fn square_world(file: u8, rank: u8) -> Vec3 {
    Vec3::new(
        file as f32 * SQUARE_SIZE - BOARD_HALF + SQUARE_SIZE * 0.5,
        0.0,
        BOARD_HALF - (rank as f32 * SQUARE_SIZE) - SQUARE_SIZE * 0.5,
    )
}

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

pub fn spawn_mini_lights(mut commands: Commands) {
    // Moody, dim lighting so the mini showcase reads like a study board
    // rather than a bright ad. Values tuned low to keep the pieces
    // atmospheric while still legible.
    commands.spawn((
        DirectionalLight {
            illuminance: 2_800.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.5, 0.0)),
        RenderLayers::layer(MINI_LAYER),
        DespawnOnExit(GameState::MainMenu),
        Name::new("XFAnimate Key Light"),
    ));

    commands.spawn((
        PointLight {
            intensity: 600.0,
            range: 18.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.5, 4.5, 3.5),
        RenderLayers::layer(MINI_LAYER),
        DespawnOnExit(GameState::MainMenu),
        Name::new("XFAnimate Fill Light"),
    ));
}
