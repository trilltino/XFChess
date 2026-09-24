use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::core::{DespawnOnExit, GameState};
use crate::xf_animate::viewport::MINI_LAYER;

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
