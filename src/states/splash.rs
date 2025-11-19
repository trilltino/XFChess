//! Splash screen plugin with dramatic pyramid scene
//!
//! Displays an eye-catching pyramid scene for 1.5 seconds before transitioning
//! to the loading screen. Features:
//! - Animated pyramid structure with glowing orb
//! - Dynamic lighting with spotlights and ambient effects
//! - Orbiting camera animation
//! - Automatic state transition via timer
//!
//! # Reference
//!
//! Based on:
//! - Original pyramid scene from `src/ui/launch_menu.rs`
//! - Lighting patterns from `reference/bevy/examples/3d/lighting.rs`
//! - State transition from `reference/bevy/examples/games/game_menu.rs`

use bevy::{
    math::ops,
    prelude::*,
};
use crate::core::{GameState, StateTransitionTimer};

/// Plugin for splash screen state
///
/// Manages the dramatic pyramid scene shown on application startup.
/// Automatically transitions to Loading state after timer expires.
pub struct SplashPlugin;

impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        app
            // Insert the transition timer resource
            .init_resource::<StateTransitionTimer>()
            // Setup the pyramid scene when entering splash state
            .add_systems(OnEnter(GameState::Splash), (
                setup_splash_camera,
                setup_pyramid_scene,
            ))
            // Update camera orbit and check timer
            .add_systems(Update, (
                update_camera_orbit,
                check_transition_timer,
            ).run_if(in_state(GameState::Splash)));
    }
}

/// Marker component for splash screen camera
#[derive(Component)]
struct SplashCamera;

/// Setup camera for splash screen with orbit animation
fn setup_splash_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        // Start at initial orbit position
        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Add fog for atmospheric effect
        DistanceFog {
            color: Color::srgb(0.02, 0.02, 0.05), // Dark blue fog
            falloff: FogFalloff::Linear {
                start: 20.0,
                end: 50.0,
            },
            ..default()
        },
        SplashCamera,
        DespawnOnExit(GameState::Splash),
        Name::new("Splash Camera"),
    ));
}

/// Setup the dramatic pyramid scene
fn setup_pyramid_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Stone material for pyramid blocks
    let stone = materials.add(StandardMaterial {
        base_color: Srgba::hex("28221B")
            .expect("hardcoded hex color '28221B' is valid")
            .into(),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });

    // Four pillars surrounding the pyramid
    for (x, z) in &[(-1.5, -1.5), (1.5, -1.5), (1.5, 1.5), (-1.5, 1.5)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(*x, 1.5, *z),
            DespawnOnExit(GameState::Splash),
            Name::new("Pyramid Pillar"),
        ));
    }

    // Glowing orb at the top
    commands.spawn((
        Mesh3d(meshes.add(Sphere::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("126212CC")
                .expect("hardcoded hex color '126212CC' is valid")
                .into(),
            reflectance: 1.0,
            perceptual_roughness: 0.0,
            metallic: 0.5,
            emissive: LinearRgba::new(0.1, 0.6, 0.1, 1.0), // Green glow
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1.75)).with_translation(Vec3::new(0.0, 4.0, 0.0)),
        DespawnOnExit(GameState::Splash),
        Name::new("Pyramid Orb"),
    ));

    // Pyramid steps (50 layers)
    for i in 0..50 {
        let half_size = i as f32 / 2.0 + 3.0;
        let y = -i as f32 / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * half_size, 0.5, 2.0 * half_size))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, y + 0.25, 0.0),
            DespawnOnExit(GameState::Splash),
            Name::new(format!("Pyramid Layer {}", i)),
        ));
    }

    // Skybox/Background
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("0a0a15") // Very dark blue
                .expect("hardcoded hex color '0a0a15' is valid")
                .into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        DespawnOnExit(GameState::Splash),
        Name::new("Skybox"),
    ));

    // === LIGHTING SETUP ===

    // Main spotlight on the orb (from above)
    commands.spawn((
        SpotLight {
            intensity: 10_000_000.0, // Very bright
            color: Color::srgb(0.2, 1.0, 0.2), // Green to match orb
            shadows_enabled: true,
            range: 30.0,
            radius: 1.0,
            inner_angle: 0.3,
            outer_angle: 0.8,
            ..default()
        },
        Transform::from_xyz(0.0, 15.0, 0.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Z),
        DespawnOnExit(GameState::Splash),
        Name::new("Orb Spotlight"),
    ));

    // Rim lights on pillars (4 point lights)
    let pillar_positions = [
        (-1.5, 3.0, -1.5),
        (1.5, 3.0, -1.5),
        (1.5, 3.0, 1.5),
        (-1.5, 3.0, 1.5),
    ];

    for (i, (x, y, z)) in pillar_positions.iter().enumerate() {
        commands.spawn((
            PointLight {
                intensity: 500_000.0,
                color: Color::srgb(1.0, 0.8, 0.5), // Warm golden light
                shadows_enabled: false,
                range: 10.0,
                radius: 0.5,
                ..default()
            },
            Transform::from_xyz(*x, *y, *z),
            DespawnOnExit(GameState::Splash),
            Name::new(format!("Pillar Light {}", i)),
        ));
    }

    // Note: Ambient light is now set per-state in main.rs or as global resource
    // For splash, we rely on directional light + point lights for ambient illumination

    // Directional light for soft shadows
    commands.spawn((
        DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            color: Color::srgb(0.9, 0.9, 1.0), // Slightly blue
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
        DespawnOnExit(GameState::Splash),
        Name::new("Directional Light"),
    ));

    info!("[SPLASH] Pyramid scene created with enhanced lighting");
}

/// Animate the camera orbiting around the pyramid
fn update_camera_orbit(
    mut camera_query: Query<&mut Transform, With<SplashCamera>>,
    time: Res<Time>,
) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let elapsed = time.elapsed_secs();

    // Orbit camera around pyramid with smooth zoom
    let orbit_scale = 8.0 + ops::sin(elapsed / 10.0) * 7.0;
    *transform = Transform::from_xyz(
        ops::cos(elapsed / 5.0) * orbit_scale,
        12.0 - orbit_scale / 2.0,
        ops::sin(elapsed / 5.0) * orbit_scale,
    )
    .looking_at(Vec3::ZERO, Vec3::Y);
}

/// Check if transition timer has expired and move to Loading state
fn check_transition_timer(
    mut next_state: ResMut<NextState<GameState>>,
    mut timer: ResMut<StateTransitionTimer>,
    time: Res<Time>,
) {
    if timer.tick(time.delta()).is_finished() {
        info!("[SPLASH] Transition timer finished, moving to Loading state");
        next_state.set(GameState::Loading);
    }
}
