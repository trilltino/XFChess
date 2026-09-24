use bevy::camera::visibility::RenderLayers;
use bevy::camera::ClearColorConfig;
use bevy::prelude::*;

use super::resource::MINI_LAYER;
use crate::core::{DespawnOnExit, GameState};

#[derive(Component)]
pub struct MiniShowcaseCamera;

pub fn spawn_mini_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 0.9,
            ..default()
        }),
        Transform::from_xyz(0.0, 9.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MINI_LAYER),
        MiniShowcaseCamera,
        DespawnOnExit(GameState::MainMenu),
        Name::new("XFAnimate Mini Camera"),
    ));
}
