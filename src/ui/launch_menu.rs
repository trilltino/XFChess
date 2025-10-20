use bevy::{
    math::ops,
    prelude::*,
};
use bevy_egui::{EguiPrimaryContextPass, EguiContexts};

use crate::ui::egui_systems::playgame_ui;
use crate::core::{GameState, LaunchMenu};

/// Wrapper for playgame_ui that handles errors
fn playgame_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<GameState>>,
    ai_config: ResMut<crate::game::ai::ChessAIResource>,
) {
    if let Err(e) = playgame_ui(contexts, next_state, ai_config) {
        error!("Error in playgame_ui: {:?}", e);
    }
}

pub fn setup_launch_camera(mut commands: Commands, _state: ResMut<NextState<GameState>>) {
    // DespawnOnExit automatically despawns this camera when exiting LaunchMenu state
    commands.spawn((
        Camera3d::default(),
        DistanceFog {
            color: Color::srgb(0.0, 0.0, 0.0),
            ..Default::default()
        },
        Name::new("Launch Menu Camera"),
        DespawnOnExit(GameState::LaunchMenu),
    ));
}

/// Alternative launch menu scene featuring a pyramid structure
///
/// This is an optional visual scene that can be used instead of the default
/// launch menu background. To enable it, add this system to OnEnter(GameState::LaunchMenu)
/// instead of setup_launch_camera.
///
/// # Usage
/// ```ignore
/// .add_systems(OnEnter(GameState::LaunchMenu), setup_pyramid_scene)
/// ```
#[allow(dead_code)] // Optional alternative scene - enable by adding to OnEnter(GameState::LaunchMenu)
pub fn setup_pyramid_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stone = materials.add(StandardMaterial {
        base_color: Srgba::hex("28221B")
            .expect("hardcoded hex color '28221B' is valid")
            .into(),
        perceptual_roughness: 1.0,
        ..default()
    });
    for (x, z) in &[(-1.5, -1.5), (1.5, -1.5), (1.5, 1.5), (-1.5, 1.5)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
            // Handle is Clone (not Copy), need .clone() in loop
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(*x, 1.5, *z),
        ));
    }

    // orb
    commands.spawn((
        Mesh3d(meshes.add(Sphere::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("126212CC")
                .expect("hardcoded hex color '126212CC' is valid")
                .into(),
            reflectance: 1.0,
            perceptual_roughness: 0.0,
            metallic: 0.5,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1.75)).with_translation(Vec3::new(0.0, 4.0, 0.0)),
    ));

    for i in 0..50 {
        let half_size = i as f32 / 2.0 + 3.0;
        let y = -i as f32 / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * half_size, 0.5, 2.0 * half_size))),
            // Handle is Clone (not Copy), need .clone() in loop
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, y + 0.25, 0.0),
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888")
                .expect("hardcoded hex color '888888' is valid")
                .into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
}

fn update_system(
    camera: Single<&mut Transform>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();

    let mut transform = camera.into_inner();

    // Orbit camera around pyramid
    let orbit_scale = 8.0 + ops::sin(now / 10.0) * 7.0;
    *transform = Transform::from_xyz(
        ops::cos(now / 5.0) * orbit_scale,
        12.0 - orbit_scale / 2.0,
        ops::sin(now / 5.0) * orbit_scale,
    )
    .looking_at(Vec3::ZERO, Vec3::Y);
}

/// Launch menu plugin that shows the game menu before starting gameplay
///
/// The generic `S` parameter allows this plugin to work with any state type,
/// making it reusable. The `state` field is used for type parameterization
/// and future state-specific logic.
pub struct Launchmenu<S: States> {
    /// The game state this plugin is associated with (used for generic type parameter)
    #[allow(dead_code)] // Used for generic type parameter S
    pub state: S,
}

impl<S: States> Plugin for Launchmenu<S> {
    fn build(&self, app: &mut App) {
        // UI system runs in EguiPrimaryContextPass with Result type (Bevy 0.17 requirement)
        app
            .add_systems(Update, update_system.run_if(in_state(LaunchMenu)))
            .add_systems(EguiPrimaryContextPass, playgame_ui_wrapper.run_if(in_state(LaunchMenu)));
    }
}
