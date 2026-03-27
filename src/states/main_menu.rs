//! Main menu plugin with polished UI
//!
//! Displays the primary game menu with options to:
//! - Start a new game (with mode selection)
//! - Access settings
//! - View statistics
//! - Exit the application
//!
//! Features styled UI components from the theme system and
//! an optional animated 3D background scene.

use crate::assets::{
    check_asset_loading, handle_asset_loading_errors, handle_untyped_asset_loading_errors,
    start_asset_loading,
};
use crate::core::{DespawnOnExit, GameMode as CoreGameMode, GameState};
use crate::game::ai::GameMode;
#[cfg(feature = "solana")]
use crate::multiplayer::solana::addon::{CompetitiveMatchState, SolanaGameSync, SolanaWallet};
#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_create_game, spawn_join_game, spawn_lookup_game, spawn_poll_opponent_joined,
    LobbyMode, LobbyStatus,
};
use crate::ui::styles::*;
use crate::ui::system_params::MainMenuUIContext;
use bevy::color::LinearRgba;
use bevy::core_pipeline::tonemapping::Tonemapping;
#[cfg(not(target_arch = "wasm32"))]
use bevy::light::{FogVolume, VolumetricFog};
use bevy::math::ops;
use bevy::prelude::*;
use bevy_egui::{egui, EguiPrimaryContextPass};
use rand::Rng;

/// Resource tracking intro fog animation state
#[derive(Resource)]
pub struct IntroFogState {
    pub timer: Timer,
    pub initial_absorption: f32,
}

impl Default for IntroFogState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
            initial_absorption: 10.0, // Very dense black fog
        }
    }
}

/// Fallback component for web builds (simulates fog with transparent mesh)
#[derive(Component)]
struct FogFallback;

/// Plugin for main menu state
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        use super::main_menu_showcase::{
            animate_showcase_captures, animate_showcase_pieces, run_showcase_game,
            spawn_showcase_board, spawn_showcase_pieces, ShowcaseGameState,
        };

        // Common systems for all platforms
        app.add_systems(
            OnEnter(GameState::MainMenu),
            (
                setup_menu_camera,
                spawn_star_spheres,
                spawn_showcase_board,
                spawn_showcase_pieces,
                start_asset_loading,
            ),
        )
        .init_resource::<MenuExpanded>()
        .init_resource::<PlayerColorChoice>()
        .init_resource::<ShowcaseGameState>()
        .init_resource::<crate::assets::GameAssets>()
        .init_resource::<crate::assets::LoadingProgress>()
        .init_resource::<crate::assets::AssetLoadingTimer>()
        .init_resource::<CompetitiveMenuState>()
        .add_systems(
            EguiPrimaryContextPass,
            main_menu_ui_wrapper.run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(
            Update,
            (
                check_asset_loading,
                handle_asset_loading_errors,
                handle_untyped_asset_loading_errors,
                animate_menu_camera,
                ensure_menu_camera_setup,
                handle_space_to_play,
                spawn_shooting_stars,
                emit_trail_particles,
                animate_shooting_stars,
                animate_particles,
                run_showcase_game,
                animate_showcase_pieces,
                animate_showcase_captures,
            )
                .run_if(in_state(GameState::MainMenu))
                .run_if(not(in_state(crate::core::MenuState::PieceViewer))),
        );

        // Fog systems (Native: VolumetricFog, Web: Mesh Fallback)
        app.init_resource::<IntroFogState>()
            .add_systems(OnEnter(GameState::MainMenu), spawn_fog_volume)
            .add_systems(
                Update,
                animate_intro_fog
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(not(in_state(crate::core::MenuState::PieceViewer))),
            );
    }
}

/// Wrapper for main_menu_ui that handles Result
fn main_menu_ui_wrapper(mut ctx: MainMenuUIContext) {
    match main_menu_ui(&mut ctx) {
        Ok(()) => {
            // UI rendered successfully
        }
        Err(e) => {
            warn!("[MAIN_MENU] UI system error: {:?}", e);
        }
    }
}

/// Marker component for menu camera
#[derive(Component)]
struct MenuCamera;

/// Resource to track menu expansion state
#[derive(Resource, Default)]
pub struct MenuExpanded {
    pub expanded: bool,
}

/// Resource to track the player's chosen color when playing vs AI
#[derive(Resource)]
pub struct PlayerColorChoice {
    pub play_as_white: bool,
}

impl Default for PlayerColorChoice {
    fn default() -> Self {
        Self { play_as_white: true }
    }
}

#[derive(Resource)]
pub struct CompetitiveMenuState {
    pub wager_sol: f32,
    pub game_id_input: String,
}

impl Default for CompetitiveMenuState {
    fn default() -> Self {
        Self {
            wager_sol: 0.1,
            game_id_input: String::new(),
        }
    }
}

/// Component for shooting star head
#[derive(Component)]
struct ShootingStar {
    velocity: Vec3,
    lifetime: Timer,
}

/// Component for individual trail particles
#[derive(Component)]
struct TrailParticle {
    lifetime: Timer,
    initial_scale: Vec3,
}

/// Component to emit particles
#[derive(Component)]
struct ParticleEmitter {
    rate_per_second: f32,
    accumulator: f32,
}

/// System to spawn shooting stars periodically
fn spawn_shooting_stars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _time: Res<Time>,
) {
    let mut rng = rand::rng();

    // Low chance to spawn per frame, adjusted for delta time
    if rng.random_bool(0.01) {
        let spawn_distance = 400.0;

        // Random start position high up and far away
        let start_pos = Vec3::new(
            rng.random_range(-spawn_distance..spawn_distance),
            rng.random_range(100.0..300.0), // High up
            rng.random_range(-spawn_distance..spawn_distance),
        );

        // Calculate velocity (moving downwards and across)
        let end_pos = Vec3::new(
            rng.random_range(-100.0..100.0),
            rng.random_range(-50.0..50.0),
            rng.random_range(-100.0..100.0),
        );

        // Fast speed for the head
        let velocity = (end_pos - start_pos).normalize() * rng.random_range(200.0..350.0);

        // Head mesh (small glowing sphere)
        let mesh = meshes.add(Sphere::new(0.5)); // Smaller head
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(5.0, 5.0, 10.0, 1.0), // Bright bluish-white
            unlit: true,
            ..default()
        });

        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(start_pos),
            ShootingStar {
                velocity,
                lifetime: Timer::from_seconds(2.5, TimerMode::Once),
            },
            ParticleEmitter {
                rate_per_second: 60.0, // High emission rate for smooth trail
                accumulator: 0.0,
            },
            DespawnOnExit(GameState::MainMenu),
            Name::new("Shooting Star Head"),
        ));
    }
}

/// System to emit trail particles from moving shooting stars
fn emit_trail_particles(
    mut commands: Commands,
    mut query: Query<(&Transform, &mut ParticleEmitter), With<ShootingStar>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let mut rng = rand::rng();

    // Shared material for particles (optimization: could be cached resource)
    let particle_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(2.0, 2.0, 8.0, 1.0), // Slightly dimmer blue trail
        unlit: true,
        ..default()
    });
    let particle_mesh = meshes.add(Sphere::new(0.3));

    for (transform, mut emitter) in query.iter_mut() {
        emitter.accumulator += emitter.rate_per_second * time.delta_secs();

        while emitter.accumulator >= 1.0 {
            emitter.accumulator -= 1.0;

            // Spawn particle at current position with slight randomization
            let jitter = Vec3::new(
                rng.random_range(-0.5..0.5),
                rng.random_range(-0.5..0.5),
                rng.random_range(-0.5..0.5),
            );

            let scale = Vec3::splat(rng.random_range(0.8..1.2));

            commands.spawn((
                Mesh3d(particle_mesh.clone()),
                MeshMaterial3d(particle_material.clone()),
                Transform {
                    translation: transform.translation + jitter,
                    scale,
                    rotation: Quat::IDENTITY,
                },
                TrailParticle {
                    lifetime: Timer::from_seconds(0.8, TimerMode::Once), // Short trail life
                    initial_scale: scale,
                },
                DespawnOnExit(GameState::MainMenu),
                Name::new("Trail Particle"),
            ));
        }
    }
}

/// Animate shooting star head movement
fn animate_shooting_stars(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut ShootingStar)>,
) {
    for (entity, mut transform, mut star) in query.iter_mut() {
        star.lifetime.tick(time.delta());

        if star.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        // Move head
        transform.translation += star.velocity * time.delta_secs();
    }
}

/// Animate trail particles (fade out and shrink)
fn animate_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut TrailParticle)>,
) {
    for (entity, mut transform, mut particle) in query.iter_mut() {
        particle.lifetime.tick(time.delta());

        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        // Shrink over lifetime
        let fraction = particle.lifetime.fraction(); // 0.0 (start) to 1.0 (end)
                                                     // Bevy Timer fraction goes 0.0 -> 1.0.
                                                     // We want scale 1.0 -> 0.0
        let scale_mult = 1.0 - fraction;

        transform.scale = particle.initial_scale * scale_mult;
    }
}

/// System to handle spacebar input for toggling menu
fn handle_space_to_play(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu_expanded: ResMut<MenuExpanded>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        menu_expanded.expanded = !menu_expanded.expanded;
        info!(
            "[MAIN_MENU] Space pressed - menu expanded: {}",
            menu_expanded.expanded
        );
    }
}

/// Setup camera for main menu with pyramid scene in background
/// Uses the persistent Egui camera and updates its transform
/// Handles case where camera might not exist yet (if OnEnter runs before PreStartup)
fn setup_menu_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<MenuCamera>),
    >,
) {
    debug!("[MAIN_MENU] Setting up menu camera with pyramid scene");
    debug!(
        "[MAIN_MENU] DEBUG: Persistent camera entity: {:?}",
        persistent_camera.entity
    );

    // Update persistent camera transform for menu view
    // Handle gracefully if camera doesn't exist yet (OnEnter runs before PreStartup for default state)
    if let Some(camera_entity) = persistent_camera.entity {
        debug!(
            "[MAIN_MENU] DEBUG: Attempting to query camera entity {:?}",
            camera_entity
        );
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                debug!("[MAIN_MENU] DEBUG: Successfully queried camera transform");
                // Closer camera angle looking down at pyramid to hide floating edge
                *transform = Transform::from_xyz(5.0, 8.0, 5.0)
                    .looking_at(Vec3::new(0.0, -2.0, 0.0), Vec3::Y);
                debug!("[MAIN_MENU] Updated persistent camera transform for menu");

                // Add menu marker and volumetric fog to persistent camera
                debug!("[MAIN_MENU] DEBUG: Adding components to camera entity");

                #[cfg(not(target_arch = "wasm32"))]
                commands.entity(camera_entity).insert((
                    MenuCamera,
                    Tonemapping::TonyMcMapface,
                    VolumetricFog {
                        ambient_intensity: 0.0,
                        ..default()
                    },
                ));

                #[cfg(target_arch = "wasm32")]
                commands
                    .entity(camera_entity)
                    .insert((MenuCamera, Tonemapping::TonyMcMapface));

                debug!("[MAIN_MENU] DEBUG: Components added successfully");
            }
            Err(e) => {
                error!(
                    "[MAIN_MENU] ERROR: Persistent camera entity {:?} exists but query failed: {:?}",
                    camera_entity, e
                );
                error!("[MAIN_MENU] ERROR: Camera may not be ready yet. Will retry in Update.");
                error!("[MAIN_MENU] ERROR: Query filter: With<PrimaryEguiContext>, Without<MenuCamera>");
            }
        }
    } else {
        debug!("[MAIN_MENU] WARNING: Persistent camera not yet created (OnEnter runs before PreStartup). Will be set up in PreStartup.");
        debug!("[MAIN_MENU] WARNING: This is expected behavior for the default state.");
    }

    // === PYRAMID SCENE ===

    // Stone material for pyramid blocks - sandstone color
    let stone = materials.add(StandardMaterial {
        base_color: Srgba::hex("C4A35A") // Sandstone/golden color
            .expect("hardcoded hex color 'C4A35A' is valid")
            .into(),
        perceptual_roughness: 0.9, // Mostly matte
        metallic: 0.0,
        reflectance: 0.3, // Some reflectance to catch light
        ..default()
    });

    // Pyramid steps (20 layers to reduce draw calls)
    for i in 0..20 {
        let half_size = i as f32 / 2.0 + 3.0;
        let y = -i as f32 / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * half_size, 0.5, 2.0 * half_size))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, y + 0.25, 0.0),
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Pyramid Layer {}", i)),
        ));
    }

    // Skybox/Background - Pure black (user requested complete black, no bluish hue)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 0.0, 0.0), // Pure black RGB(0, 0, 0)
            emissive: LinearRgba::BLACK,            // Ensure no emission
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Skybox"),
    ));

    // Directional light for soft shadows
    commands.spawn((
        DirectionalLight {
            // Increased illuminance significantly for reflections
            illuminance: 15_000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 1.0, 1.0), // Pure white
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Directional Light"),
    ));

    // Overhead light to illuminate the pyramid (VERY BRIGHT)
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0, // Very bright to illuminate dark pyramid
            range: 100.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 1.0, 1.0), // Pure white
            ..default()
        },
        Transform::from_xyz(0.0, 8.0, 0.0), // Closer to pyramid
        DespawnOnExit(GameState::MainMenu),
        Name::new("Pyramid Overhead Light"),
    ));

    // Visible Glowing Orb at the light source
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(5.0, 5.0, 5.0, 1.0), // Very bright emission
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 15.0, 0.0),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Pyramid Overhead Orb"),
    ));

    // Fill Light to ensure pyramid sides are visible
    commands.spawn((
        PointLight {
            intensity: 200_000.0, // 4x brighter
            range: 150.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 1.0, 1.0), // Pure White
            ..default()
        },
        Transform::from_xyz(20.0, 10.0, 20.0),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Pyramid Fill Light"),
    ));

    // Additional Fill Light from opposite side
    commands.spawn((
        PointLight {
            intensity: 200_000.0,
            range: 150.0,
            shadows_enabled: false,
            color: Color::srgb(1.0, 1.0, 1.0),
            ..default()
        },
        Transform::from_xyz(-20.0, 10.0, -20.0),
        DespawnOnExit(GameState::MainMenu),
        Name::new("Pyramid Fill Light 2"),
    ));

    debug!("[MAIN_MENU] Pyramid scene and camera setup complete");
}

/// Spawn the fog volume entity for intro reveal effect
fn spawn_fog_volume(
    mut commands: Commands,
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))] meshes: ResMut<Assets<Mesh>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))] materials: ResMut<
        Assets<StandardMaterial>,
    >,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native: Spawn a large fog volume covering the scene
        commands.spawn((
            FogVolume {
                absorption: 2.0, // High initial absorption for black fog
                ..default()
            },
            Transform::from_scale(Vec3::splat(100.0)),
            DespawnOnExit(GameState::MainMenu),
            Name::new("Intro Fog Volume"),
        ));
        debug!("[MAIN_MENU] Spawned intro fog volume (Volumetric)");
    }

    #[cfg(target_arch = "wasm32")]
    {
        // Web: Spawn a giant black sphere that fades out
        // Note: We use a sphere so it covers the camera from all angles
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            alpha_mode: AlphaMode::Blend,
            unlit: true, // Black unlit = pure darkness
            ..default()
        });

        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(1.0))),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(100.0)), // Giant sphere containing camera
            FogFallback,
            DespawnOnExit(GameState::MainMenu),
            Name::new("Intro Fog Fallback"),
        ));
        debug!("[MAIN_MENU] Spawned intro fog fallback (Mesh)");
    }
}

/// Animate the intro fog fading away to reveal the scene
fn animate_intro_fog(
    time: Res<Time>,
    mut fog_state: ResMut<IntroFogState>,
    #[cfg(not(target_arch = "wasm32"))] mut fog_query: Query<&mut FogVolume>,
    #[cfg(target_arch = "wasm32")] mut fallback_query: Query<(
        &MeshMaterial3d<StandardMaterial>,
        &FogFallback,
    )>,
    #[cfg(target_arch = "wasm32")] mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if fog_state.timer.is_finished() {
        return;
    }

    fog_state.timer.tick(time.delta());
    let progress = fog_state.timer.fraction();

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Lerp absorption from initial value to near-zero
        let target_absorption = fog_state.initial_absorption * (1.0 - progress);
        for mut fog in fog_query.iter_mut() {
            fog.absorption = target_absorption;
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // Lerp alpha from 1.0 (black) to 0.0 (transparent)
        // 1.0 - progress because progress goes 0->1
        let start_alpha = 1.0;
        let target_alpha = start_alpha * (1.0 - progress);

        for (mat_handle, _) in fallback_query.iter() {
            if let Some(mat) = materials.get_mut(&mat_handle.0) {
                mat.base_color.set_alpha(target_alpha);
            }
        }
    }
}

/// Spawn randomly positioned star spheres with bright white lights
/// Creates a starfield effect "as far as the eye can see" in the black foggy space
/// Spawn randomly positioned star spheres with bright white lights
/// Creates a starfield effect "as far as the eye can see" in the black foggy space
fn spawn_star_spheres(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::rng();
    // Increased to 2500 stars for deep space density
    let num_stars = 2500;

    // White emissive material for stars
    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),           // Pure white
        emissive: LinearRgba::new(10.0, 10.0, 10.0, 1.0), // VERY Bright white glow (boosted for fog)
        unlit: true,                                      // Always visible regardless of lighting
        ..default()
    });

    // Material for fake volumetric glow on Web
    #[cfg(target_arch = "wasm32")]
    let glow_material = materials.add(StandardMaterial {
        base_color: Color::hsla(0.0, 0.0, 1.0, 0.1), // White with low alpha
        alpha_mode: AlphaMode::Add,                  // Additive blending for glow
        unlit: true,
        ..default()
    });
    #[cfg(target_arch = "wasm32")]
    let glow_mesh = meshes.add(Sphere::new(1.0));

    // Spawn stars across a vast area
    for i in 0..num_stars {
        // Random positions with a huge "safe zone" for deep space feel
        // Rejection sampling to keep stars extremely far away
        let (x, y, z) = loop {
            let x = rng.random_range(-4000.0..4000.0);
            let y = rng.random_range(-2000.0..2000.0); // Flatter galaxy-like spread
            let z = rng.random_range(-4000.0..4000.0);

            // Keep stars at least 800 units away from center
            if Vec3::new(x, y, z).length() > 800.0 {
                break (x, y, z);
            }
        };

        // Very large stars to be visible at this extreme distance
        let radius = rng.random_range(3.0..6.0);

        let mut star_cmds = commands.spawn((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(star_material.clone()),
            Transform::from_xyz(x, y, z),
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Star {}", i)),
        ));

        // Add "Hero" effects to 5% of stars (closest or largest ones)
        if i % 20 == 0 {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Native: Real PointLight for volumetric fog interaction
                // Note: Range must be large to be seen in thick fog
                star_cmds.with_children(|parent| {
                    parent.spawn(PointLight {
                        intensity: 1_000_000.0, // High intensity due to distance and fog
                        range: 500.0,
                        radius: radius * 2.0, // Soft shadow radius
                        color: Color::WHITE,
                        shadows_enabled: false, // Too expensive for many stars
                        ..default()
                    });
                });
            }

            #[cfg(target_arch = "wasm32")]
            {
                // Web: Fake volumetric glow (large transparent additive sphere)
                star_cmds.with_children(|parent| {
                    parent.spawn((
                        Mesh3d(glow_mesh.clone()),
                        MeshMaterial3d(glow_material.clone()),
                        Transform::from_scale(Vec3::splat(15.0)), // Giant glow radius relative to star
                    ));
                });
            }
        }
    }

    debug!("[MAIN_MENU] Spawned {} star spheres", num_stars);
}

/// Ensure menu camera is set up if it wasn't ready during OnEnter
/// This handles the case where OnEnter runs before PreStartup (for default state)
fn ensure_menu_camera_setup(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        (&mut Transform, Option<&mut Tonemapping>),
        (With<bevy_egui::PrimaryEguiContext>, Without<MenuCamera>),
    >,
    mut commands: Commands,
    menu_camera_query: Query<Entity, With<MenuCamera>>,
) {
    // Only set up if camera exists and menu camera marker is not present
    if menu_camera_query.is_empty() {
        debug!("[MAIN_MENU] DEBUG: MenuCamera marker not found, attempting late setup");
        if let Some(camera_entity) = persistent_camera.entity {
            debug!(
                "[MAIN_MENU] DEBUG: Persistent camera entity: {:?}",
                camera_entity
            );
            match camera_query.get_mut(camera_entity) {
                Ok((mut transform, _)) => {
                    // Closer camera angle looking down at pyramid to hide floating edge
                    *transform = Transform::from_xyz(5.0, 8.0, 5.0)
                        .looking_at(Vec3::new(0.0, -2.0, 0.0), Vec3::Y);
                    debug!("[MAIN_MENU] Late setup: Updated persistent camera transform for menu");

                    #[cfg(not(target_arch = "wasm32"))]
                    commands.entity(camera_entity).insert((
                        MenuCamera,
                        Tonemapping::TonyMcMapface,
                        VolumetricFog {
                            ambient_intensity: 0.0,
                            ..default()
                        },
                    ));

                    #[cfg(target_arch = "wasm32")]
                    commands
                        .entity(camera_entity)
                        .insert((MenuCamera, Tonemapping::TonyMcMapface));

                    debug!("[MAIN_MENU] DEBUG: Late camera setup completed successfully");
                }
                Err(e) => {
                    error!(
                        "[MAIN_MENU] ERROR: Late setup failed to query camera: {:?}",
                        e
                    );
                    error!("[MAIN_MENU] ERROR: Camera entity: {:?}", camera_entity);
                }
            }
        } else {
            debug!("[MAIN_MENU] WARNING: Late setup attempted but persistent camera entity is None");
        }
    }
}

/// Animate the camera orbiting around the pyramid
fn animate_menu_camera(mut camera_query: Query<&mut Transform, With<MenuCamera>>, time: Res<Time>) {
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

/// Main menu UI system
fn main_menu_ui(ctx: &mut MainMenuUIContext) -> Result<(), bevy::ecs::query::QuerySingleError> {
    // Only show main menu UI when in a known MenuState (not PieceViewer which has its own system usually,
    // but assuming PieceViewer is handled elsewhere or sharing this?)
    // Actually, PieceViewer is a substate of MainMenu, so it might need its own UI or exit button.
    // For now we handle Main, ModeSelect, About.

    // Check if we are in a valid substate
    let current_substate = if let Some(ref menu_state_res) = ctx.current_menu_state {
        *menu_state_res.get()
    } else {
        // Default to Main if state not found (shouldn't happen)
        crate::core::MenuState::Main
    };

    // If in PieceViewer, return early (assuming separate system handles it, or just "back" button needed)
    // The previous code returned if not Main. Now we want to handle ModeSelect and About too.
    if current_substate == crate::core::MenuState::PieceViewer {
        // Maybe render a "Back" button overlay for PieceViewer?
        // For now, let's stick to the core task: Main, ModeSelect, About.
        return Ok(());
    }

    // Clone the egui Context (Arc-backed, cheap) so we don't hold a mutable borrow
    // on ctx.contexts across the closures that also need &mut ctx.
    let egui_ctx = ctx.contexts.ctx_mut()?.clone();

    // Audio Mute Toggle Button (Top-Right Corner)
    egui::Area::new("audio_toggle_area".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-20.0, 20.0))
        .show(&egui_ctx, |ui| {
            let icon = if ctx.settings.muted { "🔇" } else { "🔊" };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(icon).size(24.0))
                        .frame(false)
                        .fill(egui::Color32::TRANSPARENT),
                )
                .clicked()
            {
                ctx.settings.muted = !ctx.settings.muted;
                info!("[MAIN_MENU] Audio mute toggled: {}", ctx.settings.muted);
            }
        });

    // Bottom panel for "PRESS SPACE TO PLAY" flashing text (centered at bottom)
    if !ctx.menu_expanded.expanded {
        egui::TopBottomPanel::bottom("press_space_panel")
            .frame(egui::Frame {
                fill: egui::Color32::TRANSPARENT,
                inner_margin: egui::Margin::symmetric(0, 20),
                ..Default::default()
            })
            .show_separator_line(false)
            .show(&egui_ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Flashing "PRESS SPACE TO PLAY" text
                    let time = ui.input(|i| i.time);
                    let alpha = ((time * 2.0).sin() * 0.5 + 0.5) as f32; // Oscillates between 0.0 and 1.0
                    let color =
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8);
                    ui.label(
                        egui::RichText::new("PRESS SPACE TO PLAY")
                            .size(20.0)
                            .color(color)
                            .strong(),
                    );
                });
            });
    }

    // Only show menu content if expanded - left side panel
    if ctx.menu_expanded.expanded {
        egui::SidePanel::left("main_menu_panel")
            .resizable(false)
            .frame(egui::Frame {
                fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                inner_margin: egui::Margin::same(20),
                ..Default::default()
            })
            .show(&egui_ctx, |ui| {
                // Show loading screen if assets aren't loaded yet
                if !ctx.loading_progress.complete {
                    render_loading_screen(ui, ctx);
                    return;
                }

                // "PRESS SPACE TO HIDE" hint at top
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("PRESS SPACE TO HIDE")
                                .size(14.0)
                                .color(egui::Color32::from_rgb(150, 150, 150)),
                        );
                    });
                });

                // Main Content Area
                ui.vertical_centered(|ui| match current_substate {
                    crate::core::MenuState::Main => {
                        ui_main(ui, ctx);
                    }
                    crate::core::MenuState::ModeSelect => {
                        ui_mode_select(ui, ctx);
                    }
                    crate::core::MenuState::BraidLobby => {
                        ui_braid_lobby(ui, ctx);
                    }
                    #[cfg(feature = "solana")]
                    crate::core::MenuState::SolanaLobby => {
                        ui_solana_lobby(ui, ctx);
                    }
                    crate::core::MenuState::About => {
                        ui_about(ui, ctx);
                    }
                    _ => {}
                });
            });
    }

    Ok(())
}

fn render_loading_screen(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::small_space(ui);

        // Check if loading failed
        if ctx.loading_progress.failed {
            // Error state
            ui.heading(
                egui::RichText::new("Asset Loading Failed")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(220, 50, 50)),
            );

            Layout::small_space(ui);

            // Error message
            if let Some(ref error_msg) = ctx.loading_progress.error_message {
                ui.label(
                    egui::RichText::new(error_msg)
                        .size(12.0)
                        .color(egui::Color32::from_rgb(220, 150, 150)),
                );
            } else {
                ui.label(
                    egui::RichText::new("Failed to load required assets")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(220, 150, 150)),
                );
            }

            Layout::small_space(ui);

            // Option to continue anyway
            if ui.button("Continue Anyway (May cause issues)").clicked() {
                warn!("[MAIN_MENU] User chose to continue despite asset loading failure");
                ctx.loading_progress.complete = true;
                ctx.loading_progress.progress = 1.0;
                ctx.game_assets.loaded = true;
                info!("[MAIN_MENU] Asset loading marked as complete despite failure");
            }
        } else {
            // Loading state
            ui.heading(
                egui::RichText::new("Loading...")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(220, 220, 220)),
            );

            Layout::small_space(ui);

            // Progress bar
            let progress_bar = egui::ProgressBar::new(ctx.loading_progress.progress)
                .desired_width(300.0)
                .show_percentage()
                .animate(true);

            ui.add(progress_bar);

            Layout::small_space(ui);

            // Status text
            ui.label(
                egui::RichText::new("Loading assets...")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(180, 180, 180)),
            );
        }

        Layout::small_space(ui);
    });
}

// === SUB-MENUS ===

fn ui_main(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        // Plain text menu items - no boxes
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("PLAY")
                        .size(28.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
        }

        Layout::item_space(ui);

        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("PIECE VIEWER")
                        .size(22.0)
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            ctx.menu_state.set(crate::core::MenuState::PieceViewer);
            ctx.menu_expanded.expanded = false;
        }

        Layout::small_space(ui);

        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("ABOUT")
                        .size(22.0)
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            ctx.menu_state.set(crate::core::MenuState::About);
        }

        Layout::item_space(ui);

        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("EXIT")
                        .size(18.0)
                        .color(egui::Color32::from_rgb(200, 100, 100)),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            std::process::exit(0);
        }
    });

    Layout::small_space(ui);
    ui.label(
        egui::RichText::new("v0.1.0 - Bevy 0.17.3")
            .size(12.0)
            .color(egui::Color32::from_rgb(100, 100, 100)),
    );
}

fn ui_mode_select(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        // Back button as text
        if ui
            .add(
                egui::Label::new(
                    egui::RichText::new("← BACK")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                )
                .sense(egui::Sense::click()),
            )
            .clicked()
        {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SELECT GAME MODE")
                .size(24.0)
                .color(egui::Color32::WHITE)
                .strong(),
        );

        Layout::item_space(ui);

        // --- LOCAL PLAY ---
        ui.label(
            egui::RichText::new("LOCAL PLAY")
                .size(20.0)
                .color(egui::Color32::from_rgb(200, 200, 200))
                .strong(),
        );
        Layout::small_space(ui);

        if ui.button("♟ VS Local Friend (PvP)").clicked() {
            ctx.ai_config.mode = GameMode::Multiplayer;
            *ctx.core_mode = CoreGameMode::MultiplayerLocal;
            ctx.next_state.set(GameState::InGame);
            ctx.menu_expanded.expanded = false;
            info!("[MAIN_MENU] Starting Local PvP game");
        }

        Layout::small_space(ui);

        ui.horizontal(|ui| {
            ui.label("Play as:");
            if ui.selectable_label(ctx.color_choice.play_as_white, "♔ White").clicked() {
                ctx.color_choice.play_as_white = true;
            }
            if ui.selectable_label(!ctx.color_choice.play_as_white, "♚ Black").clicked() {
                ctx.color_choice.play_as_white = false;
            }
        });

        Layout::small_space(ui);

        let ai_color = if ctx.color_choice.play_as_white {
            crate::rendering::pieces::PieceColor::Black
        } else {
            crate::rendering::pieces::PieceColor::White
        };

        ui.horizontal(|ui| {
            ui.label("VS Stockfish:");
            for (label, difficulty) in [
                ("Easy", crate::game::ai::resource::AIDifficulty::Easy),
                ("Medium", crate::game::ai::resource::AIDifficulty::Medium),
                ("Hard", crate::game::ai::resource::AIDifficulty::Hard),
            ] {
                if ui.button(label).clicked() {
                    ctx.ai_config.mode = GameMode::VsAI { ai_color };
                    ctx.ai_config.difficulty = difficulty;
                    *ctx.core_mode = CoreGameMode::SinglePlayer;
                    ctx.next_state.set(GameState::InGame);
                    ctx.menu_expanded.expanded = false;
                    info!("[MAIN_MENU] Starting VS Stockfish ({:?}) - AI plays {:?}", difficulty, ai_color);
                }
            }
        });

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);

        #[cfg(feature = "solana")]
        {
            // --- SOLANA P2P ---
            ui.label(
                egui::RichText::new("SOLANA P2P")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(255, 150, 100))
                    .strong(),
            );
            Layout::small_space(ui);

            if ui.button("Solana Lobby").clicked() {
                ctx.menu_state.set(crate::core::MenuState::SolanaLobby);
                info!("[MAIN_MENU] Entering Solana Lobby");
            }

            Layout::item_space(ui);
            ui.separator();
            Layout::item_space(ui);
        }

        // --- P2P CHESS (IROH/BRAID) ---
        ui.label(
            egui::RichText::new("GLOBAL P2P")
                .size(20.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .strong(),
        );
        Layout::small_space(ui);

        // Display Node ID
        ui.label(
            egui::RichText::new("Your Node ID:")
                .size(14.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );

        // Get node ID from network state
        let node_id_display = if let Some(node_id) = &ctx.network_state.node_id {
            let node_id_str = bs58::encode(node_id.as_bytes()).into_string();
            format!("{:.16}...", node_id_str) // Show first 16 chars
        } else {
            "Initializing...".to_string()
        };

        ui.label(
            egui::RichText::new(&node_id_display)
                .size(16.0)
                .color(egui::Color32::from_rgb(100, 200, 255))
                .monospace(),
        );

        if ui.button("📋 Copy Full Node ID").clicked() {
            if let Some(node_id) = &ctx.network_state.node_id {
                let full_node_id = bs58::encode(node_id.as_bytes()).into_string();
                ui.output_mut(|o| {
                    o.commands
                        .push(egui::OutputCommand::CopyText(full_node_id.clone()))
                });
                info!("[MAIN_MENU] Node ID copied to clipboard: {}", full_node_id);
            }
        }

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);

        // --- HOST GAME ---
        ui.label(
            egui::RichText::new("HOST GAME")
                .size(16.0)
                .color(egui::Color32::from_rgb(100, 255, 150)),
        );
        Layout::small_space(ui);

        // Show connection status if any
        match ctx.p2p_state.status {
            crate::multiplayer::P2PConnectionStatus::Hosting => {
                ui.label(
                    egui::RichText::new("⏳ Waiting for peer to connect...")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(100, 255, 150)),
                );
            }
            crate::multiplayer::P2PConnectionStatus::Connecting => {
                ui.label(
                    egui::RichText::new("⏳ Sending invite to host...")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 200, 100)),
                );
            }
            crate::multiplayer::P2PConnectionStatus::Connected => {
                if ctx.p2p_state.is_host {
                    ui.label(
                        egui::RichText::new("✅ Peer joined! Starting game...")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(100, 255, 100))
                            .strong(),
                    );
                    if let Some(ref peer_id) = ctx.p2p_state.peer_node_id {
                        ui.label(
                            egui::RichText::new(format!(
                                "Opponent: {}...",
                                &peer_id[..peer_id.len().min(16)]
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(150, 200, 255))
                            .monospace(),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("✅ Host accepted! Game starting...")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(100, 255, 100))
                            .strong(),
                    );
                }
            }
            crate::multiplayer::P2PConnectionStatus::Error(ref msg) => {
                ui.label(
                    egui::RichText::new(format!("❌ {}", msg))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(255, 80, 80)),
                );
            }
            _ => {}
        }

        if ui.button("Start Hosting").clicked() {
            // Set AI mode to multiplayer
            ctx.ai_config.mode = GameMode::Multiplayer;
            // Emit host game event
            ctx.host_game_events
                .write(crate::multiplayer::HostGameEvent);
            *ctx.core_mode = CoreGameMode::BraidMultiplayer;
            info!("[MAIN_MENU] Hosting P2P game");
        }

        ui.label(
            egui::RichText::new("Wait for a peer to connect using your Node ID")
                .size(12.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );

        Layout::item_space(ui);
        ui.separator();
        Layout::item_space(ui);

        // --- JOIN GAME ---
        ui.label(
            egui::RichText::new("JOIN GAME")
                .size(16.0)
                .color(egui::Color32::from_rgb(255, 200, 100)),
        );
        Layout::small_space(ui);

        ui.label(
            egui::RichText::new("Enter Peer Node ID:")
                .size(14.0)
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );

        // Text input for peer node ID (persisted across frames)
        let response = ui.text_edit_singleline(&mut ctx.p2p_ui.peer_input);

        // Clear error when user starts typing
        if response.changed() {
            ctx.p2p_ui.clear_error();
        }

        // Display error message if present
        if let Some(ref error) = ctx.p2p_ui.error_message {
            Layout::small_space(ui);
            ui.label(
                egui::RichText::new(format!("⚠ {}", error))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(255, 80, 80)),
            );
        }

        Layout::small_space(ui);

        if ui.button("Connect to Peer").clicked() {
            // Validate the node ID before attempting connection
            match ctx.p2p_ui.validate_node_id() {
                Ok(()) => {
                    // Clear any previous errors
                    ctx.p2p_ui.clear_error();

                    // Set AI mode to multiplayer
                    ctx.ai_config.mode = GameMode::Multiplayer;
                    // Emit connect to peer event
                    ctx.connect_events
                        .write(crate::multiplayer::ConnectToPeerEvent {
                            peer_node_id: ctx.p2p_ui.peer_input.trim().to_string(),
                        });
                    *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                    info!(
                        "[MAIN_MENU] Joining P2P game with peer: {}",
                        ctx.p2p_ui.peer_input
                    );
                }
                Err(error_msg) => {
                    ctx.p2p_ui.set_error(error_msg);
                    warn!(
                        "[MAIN_MENU] Invalid Node ID entered: {}",
                        ctx.p2p_ui.peer_input
                    );
                }
            }
        }
    });
}

fn ui_braid_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("BRAID P2P LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(180, 120, 255))
                .strong(),
        );

        Layout::item_space(ui);

        ui.group(|ui| {
            ui.label("Base URL:");
            ui.text_edit_singleline(&mut ctx.braid_config.base_url);

            Layout::small_space(ui);

            ui.label("Game ID:");
            ui.text_edit_singleline(&mut ctx.braid_config.game_id);
        });

        Layout::item_space(ui);

        if ui.button("CONNECT & PLAY").clicked() {
            ctx.braid_config.active = true;
            *ctx.core_mode = CoreGameMode::BraidMultiplayer;
            ctx.next_state.set(GameState::InGame);
            ctx.menu_expanded.expanded = false;
        }

        Layout::item_space(ui);
        ui.label(
            egui::RichText::new(
                "Braid protocol uses decentralized HTTP for real-time state synchronization.",
            )
            .size(10.0)
            .color(egui::Color32::from_rgb(150, 150, 150)),
        );
    });
}

#[cfg(feature = "solana")]
fn ui_solana_lobby(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let Some(ref mut lobby) = ctx.solana_lobby else {
        ui.label("Solana lobby not available.");
        return;
    };

    ui.vertical_centered(|ui| {
        Layout::section_space(ui);

        if ui.button("⬅ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::ModeSelect);
            lobby.status = LobbyStatus::Idle;
        }

        Layout::section_space(ui);

        ui.label(
            egui::RichText::new("SOLANA WAGER LOBBY")
                .size(24.0)
                .color(egui::Color32::from_rgb(255, 150, 100))
                .strong(),
        );

        Layout::item_space(ui);

        // Wallet / balance header
        let balance = lobby.cached_balance;
        let wallet_ready = lobby.cached_keypair_bytes.is_some();
        if wallet_ready {
            ui.label(
                egui::RichText::new(format!("Wallet balance: {:.4} SOL", balance))
                    .color(egui::Color32::GOLD),
            );
        } else {
            ui.colored_label(egui::Color32::YELLOW, "Wallet not connected");
            if ui.button("🔗 Connect Wallet").clicked() {
                crate::multiplayer::solana::tauri_signer::open_wallet_browser();
            }
        }

        // Node ID display
        if let Some(node_id) = &ctx.network_state.node_id {
            let node_id_str = bs58::encode(node_id.as_bytes()).into_string();
            let short_id = if node_id_str.len() > 16 {
                format!("{:.16}...", node_id_str)
            } else {
                node_id_str.clone()
            };
            ui.label(
                egui::RichText::new(format!("Node ID: {}", short_id))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(100, 200, 255))
                    .monospace(),
            );
            if ui.small_button("📋 Copy Node ID").clicked() {
                ui.output_mut(|o| {
                    o.commands
                        .push(egui::OutputCommand::CopyText(node_id_str.clone()));
                });
                info!("[SOLANA_LOBBY] Node ID copied: {}", node_id_str);
            }
        } else {
            ui.label(
                egui::RichText::new("Node ID: Initializing...")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        }

        Layout::item_space(ui);

        // Only show the create/join form while not in a post-transaction state.
        let in_post_state = matches!(
            lobby.status,
            LobbyStatus::WaitingForOpponent { .. }
                | LobbyStatus::OpponentJoined { .. }
                | LobbyStatus::Success(_)
        );

        if !in_post_state {
            // Tab switcher
            ui.horizontal(|ui| {
                if ui.selectable_label(lobby.mode == LobbyMode::Create, "Create Game").clicked() {
                    lobby.mode = LobbyMode::Create;
                    lobby.status = LobbyStatus::Idle;
                }
                if ui.selectable_label(lobby.mode == LobbyMode::Join, "Join Game").clicked() {
                    lobby.mode = LobbyMode::Join;
                    lobby.status = LobbyStatus::Idle;
                }
            });

            ui.separator();
            Layout::item_space(ui);

            match lobby.mode {
                LobbyMode::Create => render_create_tab(ui, lobby),
                LobbyMode::Join => render_join_tab(ui, lobby),
            }
        }

        Layout::item_space(ui);

        // Auto-transition: Success + Create mode → WaitingForOpponent + start poll.
        if let LobbyStatus::Success(game_id) = lobby.status {
            if lobby.mode == LobbyMode::Create && lobby.opponent_poll_rx.is_none() {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_poll_opponent_joined(lobby.cached_rpc_url.clone(), game_id, tx);
                lobby.opponent_poll_rx = Some(rx);
                lobby.status = LobbyStatus::WaitingForOpponent { game_id };
            }
        }

        // Post-action status UI (uses ctx for firing events).
        let status_snap = lobby.status.clone();
        let wager_lamports = lobby.wager_lamports();

        match status_snap {
            LobbyStatus::Idle => {}

            LobbyStatus::Pending => {
                ui.spinner();
                ui.label(
                    egui::RichText::new("⏳ Submitting transaction...")
                        .color(egui::Color32::from_rgb(200, 200, 50)),
                );
            }

            LobbyStatus::WaitingForOpponent { game_id } => {
                ui.spinner();
                ui.label(
                    egui::RichText::new(format!(
                        "⏳ Game #{} — waiting for opponent to join on-chain...",
                        game_id
                    ))
                    .color(egui::Color32::from_rgb(255, 200, 80)),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Share your Node ID with your opponent:")
                        .size(12.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                if let Some(node_id) = &ctx.network_state.node_id {
                    let full = bs58::encode(node_id.as_bytes()).into_string();
                    let short = format!("{:.16}...", full);
                    ui.label(
                        egui::RichText::new(&short)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(100, 200, 255))
                            .monospace(),
                    );
                    if ui.small_button("📋 Copy Node ID").clicked() {
                        ui.output_mut(|o| {
                            o.commands
                                .push(egui::OutputCommand::CopyText(full.clone()));
                        });
                        info!("[SOLANA_LOBBY] Node ID copied while waiting: {}", full);
                    }
                }
                Layout::small_space(ui);
                if ui.small_button("✖ Cancel").clicked() {
                    lobby.status = LobbyStatus::Idle;
                    lobby.opponent_poll_rx = None;
                }
            }

            LobbyStatus::OpponentJoined { game_id } => {
                ui.label(
                    egui::RichText::new("✅ Opponent joined on-chain!")
                        .color(egui::Color32::from_rgb(100, 255, 100))
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new(
                        "Click 'Host Game' — opponent will enter your Node ID to connect.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );
                Layout::small_space(ui);
                if ui.button("🎮 Host Game").clicked() {
                    ctx.ai_config.mode = GameMode::Multiplayer;
                    ctx.host_game_events
                        .write(crate::multiplayer::HostGameEvent);
                    *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                    if let Some(ref mut sync) = ctx.solana_sync {
                        sync.game_id = Some(game_id);
                        sync.wager_amount = wager_lamports;
                    }
                    if let Some(ref mut comp) = ctx.competitive {
                        comp.game_id = Some(game_id);
                        comp.wager_lamports = wager_lamports;
                        comp.active = true;
                    }
                    info!("[SOLANA_LOBBY] Hosting P2P for on-chain game #{}", game_id);
                }
            }

            // Join success — show Node ID entry so joiner can connect to host.
            LobbyStatus::Success(game_id) => {
                ui.label(
                    egui::RichText::new(format!("✅ Joined game #{}!", game_id))
                        .color(egui::Color32::GREEN)
                        .strong(),
                );
                Layout::small_space(ui);
                ui.label(
                    egui::RichText::new("Enter host's Node ID to start playing:")
                        .size(13.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );

                let response = ui.text_edit_singleline(&mut ctx.p2p_ui.peer_input);
                if response.changed() {
                    ctx.p2p_ui.clear_error();
                }
                if let Some(ref err) = ctx.p2p_ui.error_message.clone() {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", err))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(255, 80, 80)),
                    );
                }
                Layout::small_space(ui);
                if ui.button("🔗 Connect to Host").clicked() {
                    match ctx.p2p_ui.validate_node_id() {
                        Ok(()) => {
                            ctx.p2p_ui.clear_error();
                            ctx.ai_config.mode = GameMode::Multiplayer;
                            let peer = ctx.p2p_ui.peer_input.trim().to_string();
                            ctx.connect_events
                                .write(crate::multiplayer::ConnectToPeerEvent {
                                    peer_node_id: peer.clone(),
                                });
                            *ctx.core_mode = CoreGameMode::BraidMultiplayer;
                            if let Some(ref mut sync) = ctx.solana_sync {
                                sync.game_id = Some(game_id);
                                sync.wager_amount = wager_lamports;
                            }
                            if let Some(ref mut comp) = ctx.competitive {
                                comp.game_id = Some(game_id);
                                comp.wager_lamports = wager_lamports;
                                comp.active = true;
                            }
                            info!(
                                "[SOLANA_LOBBY] Connecting to host {} for game #{}",
                                peer, game_id
                            );
                        }
                        Err(e) => {
                            ctx.p2p_ui.set_error(e);
                        }
                    }
                }
            }

            LobbyStatus::Fetched { .. } => {}

            LobbyStatus::Error(msg) => {
                ui.colored_label(egui::Color32::RED, format!("❌ {}", msg));
                if ui.small_button("↩ Try Again").clicked() {
                    lobby.status = LobbyStatus::Idle;
                }
            }
        }
    });
}

#[cfg(feature = "solana")]
fn render_create_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    let balance = lobby.cached_balance;
    let max_wager = ((balance - 0.002) as f32).max(0.0);

    ui.label(egui::RichText::new("Wager amount (SOL)").size(14.0));

    ui.add(
        egui::Slider::new(&mut lobby.wager_sol, 0.001..=max_wager.max(0.001))
            .step_by(0.001)
            .fixed_decimals(3),
    );

    // Clamp after slider interaction
    lobby.wager_sol = lobby.wager_sol.clamp(0.001, max_wager.max(0.001));

    ui.label(
        egui::RichText::new(format!("Escrow: {:.4} SOL  |  Pot: {:.4} SOL",
            lobby.wager_sol, lobby.wager_sol * 2.0))
            .color(egui::Color32::LIGHT_GRAY)
            .size(12.0),
    );

    Layout::small_space(ui);

    let can_create = lobby.cached_keypair_bytes.is_some()
        && lobby.wager_sol >= 0.001
        && (lobby.wager_sol as f64) <= balance - 0.002
        && !matches!(lobby.status, LobbyStatus::Pending);

    if ui.add_enabled(can_create, egui::Button::new("🎮 Create Game")).clicked() {
        if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_create_game(
                lobby.cached_rpc_url.clone(),
                wallet_pubkey,
                lobby.wager_lamports(),
                tx,
            );
            lobby.tx_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Creating game with wager {} SOL", lobby.wager_sol);
        }
    }

    if !can_create && lobby.cached_keypair_bytes.is_some() && balance < 0.003 {
        ui.colored_label(egui::Color32::RED, "Insufficient balance (need ≥ 0.003 SOL)");
    }
}

#[cfg(feature = "solana")]
fn render_join_tab(
    ui: &mut egui::Ui,
    lobby: &mut crate::multiplayer::solana::lobby::SolanaLobbyState,
) {
    ui.label(egui::RichText::new("Enter Game ID:").size(14.0));
    ui.text_edit_singleline(&mut lobby.game_id_input);

    Layout::small_space(ui);

    let game_id_valid = lobby.game_id_input.trim().parse::<u64>().is_ok();
    let looking_up = matches!(lobby.status, LobbyStatus::Pending);

    if ui.add_enabled(game_id_valid && !looking_up, egui::Button::new("🔍 Look Up")).clicked() {
        if let Ok(game_id) = lobby.game_id_input.trim().parse::<u64>() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            spawn_lookup_game(lobby.cached_rpc_url.clone(), game_id, tx);
            lobby.lookup_rx = Some(rx);
            lobby.status = LobbyStatus::Pending;
            info!("[SOLANA_LOBBY] Looking up game {}", game_id);
        }
    }

    // Show fetched wager info + confirm join button
    if let LobbyStatus::Fetched { wager_sol, game_id } = lobby.status {
        ui.separator();
        Layout::small_space(ui);
        ui.label(
            egui::RichText::new(format!("Game #{} requires {:.4} SOL wager", game_id, wager_sol))
                .color(egui::Color32::GOLD),
        );
        ui.label(
            egui::RichText::new(format!("Your balance: {:.4} SOL", lobby.cached_balance))
                .size(12.0),
        );

        let sufficient = lobby.cached_balance >= wager_sol + 0.002;
        let can_join = lobby.cached_keypair_bytes.is_some() && sufficient;

        if !sufficient {
            ui.colored_label(egui::Color32::RED, "Insufficient balance to join");
        }

        if ui.add_enabled(can_join, egui::Button::new("✅ Confirm Join")).clicked() {
            if let Some(wallet_pubkey) = wallet_pubkey_from_cached(&lobby.cached_keypair_bytes) {
                let (tx, rx) = tokio::sync::oneshot::channel();
                spawn_join_game(
                    lobby.cached_rpc_url.clone(),
                    wallet_pubkey,
                    game_id,
                    tx,
                );
                // Copy fetched wager into create flow so poll_lobby_tasks can persist it
                lobby.wager_sol = wager_sol as f32;
                lobby.tx_rx = Some(rx);
                lobby.status = LobbyStatus::Pending;
                info!("[SOLANA_LOBBY] Joining game {} (wager {:.4} SOL)", game_id, wager_sol);
            }
        }
    }
}

#[cfg(feature = "solana")]
fn wallet_pubkey_from_cached(bytes: &Option<Vec<u8>>) -> Option<solana_sdk::pubkey::Pubkey> {
    let arr: [u8; 32] = bytes.as_deref()?.try_into().ok()?;
    Some(solana_sdk::pubkey::Pubkey::from(arr))
}


fn ui_about(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    Layout::small_space(ui);
    if ui.button("⬅ Back").clicked() {
        ctx.menu_state.set(crate::core::MenuState::Main);
    }
    Layout::section_space(ui);

    ui.vertical_centered(|ui| {
        ui.heading(TextStyle::heading("About XFChess", TextSize::MD));
        Layout::item_space(ui);

        ui.label(
            egui::RichText::new("An experimental chess engine built with Bevy 0.18")
                .color(egui::Color32::LIGHT_GRAY),
        );
        Layout::small_space(ui);

        ui.label("Created by XF Team");
        ui.label("Version 0.1.0");

        Layout::item_space(ui);
        ui.label(egui::RichText::new("Features:").strong());
        ui.label("- 3D Graphics & Animations");
        ui.label("- Minimax AI Agent");
        ui.label("- TempleOS Tribute Mode");
    });
}
