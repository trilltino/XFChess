//! Swappable 3D background — a large unlit dome textured with a free CC0
//! equirectangular photo (Poly Haven HDRIs, polyhaven.com/hdris) instead of
//! the old solid-black backdrop. Tagged for both the default render layer
//! (menu/persistent camera) and `BOARD_LAYER` (the dedicated in-game board
//! camera — see `game::systems::camera::BoardCamera`), so a single dome
//! entity covers both contexts.

use bevy::camera::visibility::RenderLayers;
use bevy::mesh::SphereKind;
use bevy::prelude::*;

use crate::game::systems::camera::BOARD_LAYER;

/// (display name, asset path under `assets/`) for each bundled backdrop.
/// `None` = plain black (the default — index 0). The photo options are CC0
/// from polyhaven.com/hdris — free for any use, no attribution required. See
/// `assets/environments/README.md`. To add one: drop an equirectangular
/// `.hdr` or `.jpg`/`.png` file into `assets/environments/` and add a line
/// here (both formats work — `AssetServer::load` picks the loader from the
/// extension).
pub const BACKDROP_PRESETS: &[(&str, Option<&str>)] = &[
    ("Black", None),
    ("Brown Studio", Some("environments/brown_photostudio_02_1k.hdr")),
    ("Reading Room", Some("environments/reading_room_1k.hdr")),
    ("Poly Haven Studio", Some("environments/poly_haven_studio_1k.hdr")),
    ("Wooden Lounge", Some("environments/wooden_lounge_1k.hdr")),
];

/// Radius of the background dome. Comfortably inside the default camera's
/// far plane (1000.0) with margin for camera panning/zoom.
const DOME_RADIUS: f32 = 600.0;

/// Marks the background dome mesh entity.
#[derive(Component)]
pub struct BackgroundDome;

/// Tracks which backdrop is active and holds the dome's material handle, so
/// swapping is just "load a new texture, point the material at it" — no
/// respawn needed.
#[derive(Resource)]
pub struct BackgroundEnvironment {
    pub current: u8,
    pub material: Handle<StandardMaterial>,
}

/// Plugin that spawns the background dome once at startup and keeps it in
/// sync with `GameSettings.background_env`.
pub struct EnvironmentPlugin;

impl Plugin for EnvironmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_background_dome)
            .add_systems(Update, update_background_environment_system);
    }
}

/// Spawns the background dome, replacing the old solid-black cuboid
/// (`setup_global_scene` previously owned this — see its doc comment).
/// Runs once at `Startup` and persists for the app's lifetime, same as the
/// entity it replaces.
fn setup_background_dome(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = Sphere::new(DOME_RADIUS)
        .mesh()
        .kind(SphereKind::Uv {
            sectors: 32,
            stacks: 16,
        })
        .build();

    // Default preset (index 0) is "Black" — no texture, just an unlit black
    // sphere, matching the old solid-black backdrop it replaces.
    let material = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        base_color_texture: None,
        unlit: true,
        // Camera sits inside the dome — without this the sphere's inward-
        // facing surface would be backface-culled and invisible.
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone()),
        Transform::IDENTITY,
        bevy::picking::Pickable::IGNORE,
        BackgroundDome,
        // Layer 0 (default) covers the menu/persistent camera; BOARD_LAYER
        // covers the dedicated in-game board camera, which only renders that
        // one layer (see `BoardCamera`'s doc comment).
        RenderLayers::from_layers(&[0, BOARD_LAYER]),
        Name::new("Background Dome"),
    ));

    commands.insert_resource(BackgroundEnvironment { current: 0, material });
}

/// Watches `GameSettings.background_env` and re-points the dome's material
/// at a new texture when it changes — mirrors
/// `board::board_theme::update_board_theme_system`'s change-detection shape.
fn update_background_environment_system(
    settings: Res<crate::core::GameSettings>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut env: Option<ResMut<BackgroundEnvironment>>,
    mut last_index: Local<Option<u8>>,
) {
    let Some(env) = env.as_mut() else {
        return;
    };
    let current = settings.background_env;
    if *last_index == Some(current) {
        return;
    }
    *last_index = Some(current);
    if current == env.current && materials.get(&env.material).is_some() {
        // First run after startup — dome already shows this preset.
        return;
    }

    let Some((name, path)) = BACKDROP_PRESETS.get(current as usize) else {
        return;
    };
    if let Some(mut mat) = materials.get_mut(&env.material) {
        match path {
            Some(p) => {
                mat.base_color_texture = Some(asset_server.load(*p));
                mat.base_color = Color::WHITE;
            }
            None => {
                mat.base_color_texture = None;
                mat.base_color = Color::BLACK;
            }
        }
    }
    env.current = current;
    info!("[ENVIRONMENT] Backdrop switched to {}", name);
}
