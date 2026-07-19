//! Opt-in physical sky / atmosphere for the in-game board scene.
//!
//! Gated behind the `sky` cargo feature (off by default). When enabled, this
//! adds Bevy's physically-based atmosphere to the game while a match is in
//! progress: a single `Atmosphere` "planet" entity plus per-camera
//! `AtmosphereSettings` on the shared 3D camera.
//!
//! ## Why this is opt-in, and the rc.3 caveat
//!
//! The project pins `bevy 0.19.0-rc.3`. In rc.3 the atmosphere still uses the
//! *single-camera* buffer path (`init_atmosphere_buffer`); the fully
//! multi-camera-correct rework (`AtmosphereBuffer` as a per-camera component)
//! landed *after* rc.3. XFChess runs several cameras (the persistent egui/menu
//! camera, the game camera, menu/cinematic cameras), so on rc.3 the atmosphere
//! is only guaranteed correct for the camera it is attached to. Keeping this
//! behind a feature flag means the default build is completely unaffected, and
//! the flag can be promoted to default once a Bevy release with the multi-camera
//! fix is pinned.
//!
//! rc.3's extraction already *selects per camera* — for each `Camera3d` carrying
//! `AtmosphereSettings` it picks the nearest `Atmosphere` entity — so this code
//! is written in the multi-camera shape and will not need restructuring when the
//! buffer fix lands; only the feature gate comes off.

use bevy::prelude::*;

#[cfg(feature = "sky")]
use crate::core::{DespawnOnExit, GameState};
// In bevy 0.19.0-rc.3 the atmosphere components live in `bevy_light` (`Atmosphere`,
// `ScatteringMedium`); only `AtmosphereSettings` is re-exported from `bevy::pbr`.
#[cfg(feature = "sky")]
use bevy::light::{atmosphere::ScatteringMedium, Atmosphere};
#[cfg(feature = "sky")]
use bevy::pbr::AtmosphereSettings;

/// Plugin that installs the opt-in atmosphere. A no-op unless the `sky` feature
/// is enabled, so it is always safe to add.
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

/// Spawn the atmosphere planet and attach atmosphere settings to the board camera.
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

    // Attach per-camera atmosphere settings to the dedicated board camera (the
    // one that renders the 3D world during gameplay — see `camera::BoardCamera`).
    // This also pulls in `Hdr` (required by `AtmosphereSettings`); `teardown_sky`
    // strips both back off on exit, though the board camera despawns on its own.
    if let Ok(camera) = board_camera.single() {
        commands
            .entity(camera)
            .insert(AtmosphereSettings::default());
    }
}

/// Remove atmosphere settings (and the HDR requirement) from the board camera
/// when leaving the game. The board camera itself despawns via `DespawnOnExit`,
/// so this is mostly a safety net in case it runs before that cleanup.
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
