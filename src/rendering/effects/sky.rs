use bevy::prelude::*;

#[cfg(feature = "sky")]
use crate::core::{DespawnOnExit, GameState};
// Atmosphere and ScatteringMedium live in bevy_light; only AtmosphereSettings is re-exported by pbr.
#[cfg(feature = "sky")]
use bevy::light::{atmosphere::ScatteringMedium, Atmosphere};
#[cfg(feature = "sky")]
use bevy::pbr::AtmosphereSettings;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(feature = "sky")]
        {
            _app.add_systems(OnEnter(GameState::InGame), setup_sky)
                .add_systems(OnExit(GameState::InGame), teardown_sky);
        }
    }
}

#[cfg(feature = "sky")]
fn setup_sky(
    mut commands: Commands,
    mut media: ResMut<Assets<ScatteringMedium>>,
    board_camera: Query<Entity, With<crate::game::systems::camera::BoardCamera>>,
) {
    // The medium describes how the air scatters light; `earth` gives a familiar
    // blue daytime sky. The resolutions are the values Bevy uses for Earth.
    let medium = media.add(ScatteringMedium::earth(64, 64));

    // One "planet" the cameras scatter against. `Atmosphere` requires (and
    // auto-positions) a `GlobalTransform`, so we only despawn it on exit.
    commands.spawn((
        Atmosphere::earth(medium),
        DespawnOnExit(GameState::InGame),
        Name::new("Sky Atmosphere"),
    ));

    // AtmosphereSettings requires Hdr on the board camera; teardown_sky removes both.
    if let Ok(camera) = board_camera.single() {
        commands
            .entity(camera)
            .insert(AtmosphereSettings::default());
    }
}

#[cfg(feature = "sky")]
fn teardown_sky(
    mut commands: Commands,
    board_camera: Query<Entity, With<crate::game::systems::camera::BoardCamera>>,
) {
    if let Ok(camera) = board_camera.single() {
        commands
            .entity(camera)
            .remove::<AtmosphereSettings>()
            .remove::<bevy::camera::Hdr>();
    }
}
