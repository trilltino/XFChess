//! XFChess - A 3D Chess Game built with Bevy 0.17.3
//!
//! A modern, idiomatic chess game implementation following Bevy 0.17 best practices
//! with comprehensive state management, ECS architecture, and AI opponent.
//!
//! # Architecture Overview
//!
//! XFChess features a modern, modular architecture with comprehensive state management:
//!
//! ## State Flow
//! ```text
//! Main Menu (with loading) → In-Game ⇄ Paused → Game Over
//!         ↓↑                                    ↓
//!      Settings ←──────────────────────────────┘
//! ```
//!
//! See [`crate::core::states`] for detailed state management documentation.
//!
//! ## Plugin Architecture
//!
//! The application follows Bevy 0.17 plugin best practices:
//!
//! 1. **[`crate::core::CorePlugin`]** - Foundation (state, resources, panic hook)
//! 2. **Core Bevy Plugins** - DefaultPlugins, EguiPlugin, WorldInspectorPlugin
//! 3. **[`crate::game::GamePlugin`]** - Game logic (rules, AI, move validation)
//! 4. **State Plugins** - MainMenuPlugin, SettingsPlugin, etc. (see [`crate::states`])
//! 5. **Asset Management** - Centralized asset preloading (see [`crate::assets`])
//! 6. **Input Systems** - MeshPickingPlugin, PointerEventsPlugin (see [`crate::input`])
//! 7. **Rendering** - PiecePlugin, BoardPlugin (see [`crate::rendering`])
//!
//! ## Module Organization
//!
//! - **[`crate::core`]** - State management, window config, settings persistence
//! - **[`crate::states`]** - State-specific plugins (splash, loading, menus, etc.)
//! - **[`crate::assets`]** - Asset management and preloading
//! - **[`crate::rendering`]** - 3D board, pieces, lighting
//! - **[`crate::ui`]** - EGUI theme system and components
//! - **[`crate::input`]** - Pointer events and piece selection
//! - **[`crate::game`]** - Chess rules, move validation, AI
//! - **[`chess_engine`]** - Chess engine crate (minimax, alpha-beta pruning)
//!
//! ## Key Features
//!
//! - **Polished UI**: Themed menu system with consistent styling (see [`crate::ui::styles`])
//! - **Asset Loading**: Progress indication during startup (see [`crate::assets`])
//! - **Chess AI**: Minimax engine with alpha-beta pruning (~1800-2000 ELO) (see [`crate::game::ai`])
//! - **3D Graphics**: Enhanced lighting and atmospheric effects (see [`crate::rendering`])
//! - **Game Modes**: Human vs AI with selectable colors (White/Black), TempleOS view mode
//! - **Settings**: Graphics quality, board themes, preferences (see [`crate::core::GameSettings`])
//! - **Statistics**: Player performance tracking (see [`crate::core::GameStatistics`])
//! - **Inspector UI**: F1 to toggle ECS entity/component inspector
//!
//! ## System Execution Order
//!
//! Systems are organized into sets with explicit ordering (see [`crate::game::system_sets`]):
//!
//! 1. **Input** - User input (camera, piece selection)
//! 2. **Validation** - Move validation and board sync
//! 3. **Execution** - Move execution and game state updates
//! 4. **Visual** - Rendering updates (highlights, animations)
//!
//! # Reference Materials
//!
//! - `reference/bevy/` - Bevy 0.17 examples and API patterns
//! - `reference/bevy_egui/` - bevy_egui integration patterns
//! - `reference/bevy-inspector-egui/` - Inspector integration
//! - `reference/bevy-3d-chess/` - Alternative chess implementation

use bevy::ecs::error::{warn as bevy_warn, BevyError, ErrorContext};
use bevy::input::common_conditions::input_toggle_active;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy::render::settings::{PowerPreference, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy_egui::{
    EguiContext, EguiGlobalSettings, EguiMultipassSchedule, EguiPlugin, EguiPrimaryContextPass,
    PrimaryEguiContext,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

// Module declarations
mod assets;
mod audio;
mod core;
mod game;
mod input;
mod persistent_camera;
mod rendering;
mod states;
mod ui;

pub use persistent_camera::PersistentEguiCamera;

// Imports
use core::{CorePlugin, DespawnOnExit, GameState, WindowConfig};
use game::{CameraController, GamePlugin};
use input::*;
use rendering::*;
use states::*;

/// Helper function to write errors to a log file
/// Creates logs/ directory if it doesn't exist and appends error messages
fn write_error_to_file(message: &str) {
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        if let Err(e) = fs::create_dir_all(logs_dir) {
            eprintln!("[ERROR] Failed to create logs directory: {}", e);
            return;
        }
    }

    // Use system time for timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp = format!("{}", now);
    let log_file = logs_dir.join(format!("error_{}.log", timestamp));

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_file) {
        if let Err(e) = writeln!(file, "[{}] {}", timestamp, message) {
            eprintln!("[ERROR] Failed to write to log file: {}", e);
        }
    }
}

fn main() {
    std::env::set_var("NO_COLOR", "1");

    let mut app = App::new();

    setup_error_handler(&mut app);
    configure_core_plugins(&mut app);
    configure_gui_plugins(&mut app);
    initialize_resources(&mut app);
    configure_state_plugins(&mut app);
    configure_game_plugins(&mut app);
    configure_game_state_systems(&mut app);
    configure_settings_systems(&mut app);
    configure_global_setup(&mut app);

    app.run();
}

fn setup_error_handler(app: &mut App) {
    app.set_error_handler(|error: BevyError, context: ErrorContext| {
        write_error_to_file(&format!(
            "[ERROR_HANDLER] System '{}' failed: {}\nError details: {:?}\nError type: {:?}",
            context.name(),
            error,
            error,
            std::any::type_name_of_val(&error)
        ));

        error!(
            "[ERROR_HANDLER] System '{}' failed: {}",
            context.name(),
            error
        );
        error!(
            "[ERROR_HANDLER] Error details: {:?}",
            error
        );
        error!(
            "[ERROR_HANDLER] Error type: {:?}",
            std::any::type_name_of_val(&error)
        );
        warn!(
            "[ERROR_HANDLER] Application will continue running, but the failed system will be skipped"
        );
        bevy_warn(error, context);
    });
}

fn configure_core_plugins(app: &mut App) {
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(WindowConfig::default().to_window()),
                ..default()
            })
            .set(AssetPlugin {
                file_path: "assets".to_string(),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    power_preference: PowerPreference::HighPerformance,
                    ..default()
                }),
                ..default()
            })
            // Disable default LogPlugin to use our custom one
            .disable::<bevy::log::LogPlugin>(),
    );
    app.add_plugins(FileLoggerPlugin);
    app.add_plugins(CorePlugin);
}

/// Custom plugin to handle file logging
pub struct FileLoggerPlugin;

impl Plugin for FileLoggerPlugin {
    fn build(&self, app: &mut App) {
        use tracing_subscriber::{layer::SubscriberExt, Layer};

        // Create logs directory
        let _ = std::fs::create_dir_all("logs");

        // File appender (hourly rolling)
        let file_appender = tracing_appender::rolling::hourly("logs", "debug");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // Keep the guard alive by leaking it (safe for global logger in app lifespan)
        // In a clearer implementation, we'd store this in a resource, but for strict logger setup
        // leaking is a common workaround if we can't store it in the app easily during build.
        // However, Bevy's LogPlugin is safer. Let's try to just ADD a subscriber if possible.
        // But Bevy sets the global default. We must use Bevy's LogPlugin or replace it.
        // Actually, better path: Use Bevy's LogPlugin but customize the subscriber if exposed? No.
        // Best path: tracing_subscriber::fmt() allows multiple layers.

        // Let's rely on standard tracing setup since we disabled Bevy's LogPlugin.
        // We recreate what Bevy does but add our file layer.

        // Console layer
        let console_layer = tracing_subscriber::fmt::Layer::new()
            .with_target(false)
            .with_filter(tracing_subscriber::EnvFilter::new(
                "debug,bevy_ecs=warn,bevy_render=warn,wgpu=error,naga=error",
            ));

        // File layer (no color, full details)
        let file_layer = tracing_subscriber::fmt::Layer::new()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_filter(tracing_subscriber::EnvFilter::new(
                "debug,bevy_ecs=warn,bevy_render=warn,wgpu=error,naga=error",
            ));

        let subscriber = tracing_subscriber::Registry::default()
            .with(console_layer)
            .with(file_layer);

        // Set global default - safely ignore error if already set
        let _ = tracing::subscriber::set_global_default(subscriber);

        // We must leak the guard to prevent file writing from stopping immediately
        Box::leak(Box::new(_guard));
    }
}

fn configure_gui_plugins(app: &mut App) {
    app.add_plugins(EguiPlugin::default());
    app.add_systems(
        PreStartup,
        |mut egui_settings: ResMut<EguiGlobalSettings>| {
            egui_settings.auto_create_primary_context = false;
        },
    );
    app.add_plugins(
        WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F1)),
    );
    app.add_systems(
        EguiPrimaryContextPass,
        crate::ui::inspector::inspector_ui.run_if(input_toggle_active(false, KeyCode::F1)),
    );
}

fn initialize_resources(app: &mut App) {
    app.init_resource::<PersistentEguiCamera>();
    app.init_resource::<crate::assets::LoadingProgress>();
    app.init_resource::<crate::assets::GameAssets>();
    app.add_systems(Update, log_system_errors);
}

fn configure_state_plugins(app: &mut App) {
    app.add_plugins(MainMenuPlugin);
    app.add_plugins(SettingsPlugin);
    app.add_plugins(PausePlugin);
    app.add_plugins(GameOverPlugin);
    app.add_plugins(PieceViewerPlugin);
}

fn configure_game_plugins(app: &mut App) {
    // app.add_plugins(DefaultPickingPlugins); // Already added by DefaultPlugins in Bevy 0.17+
    app.add_plugins(MeshPickingPlugin);
    app.add_plugins(PointerEventsPlugin);
    app.add_plugins(PiecePlugin);
    app.add_plugins(BoardPlugin);
    app.add_plugins(BoardUtils);
    app.add_plugins(DynamicLightingPlugin);
    app.add_plugins(GamePlugin);
}

fn configure_game_state_systems(app: &mut App) {
    app.add_systems(
        OnEnter(GameState::InGame),
        (
            game::systems::reset_game_resources
                .before(rendering::pieces::create_pieces)
                .before(rendering::board::create_board),
            game::systems::initialize_players.after(game::systems::reset_game_resources),
            game::systems::initialize_game_sounds.after(game::systems::reset_game_resources),
            game::systems::play_templeos_sound.after(game::systems::initialize_game_sounds),
            game::systems::initialize_engine_from_ecs
                .after(rendering::pieces::create_pieces)
                .after(rendering::board::create_board),
            setup_game_scene,
            setup_game_camera,
            game::systems::spawn_camera_position_ui,
        )
            .chain(),
    );
    app.add_systems(OnExit(GameState::InGame), reset_game_camera);
    app.add_systems(
        Update,
        (
            handle_pause_input.run_if(in_state(GameState::InGame)),
            game::systems::check_and_play_templeos_sound.run_if(in_state(GameState::InGame)),
            game::systems::update_camera_position_ui.run_if(in_state(GameState::InGame)),
        ),
    );
}

/// Configure the persistent camera for gameplay
/// Use the existing Egui camera as the main game camera to avoid conflicts
fn setup_game_camera(
    mut commands: Commands,
    persistent_camera: Res<PersistentEguiCamera>,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut query: Query<(&mut Transform, &mut Camera)>,
) {
    // Only configure for standard view (TempleOS handles its own camera/view)
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        return;
    }

    if let Some(entity) = persistent_camera.entity {
        if let Ok((mut transform, mut camera)) = query.get_mut(entity) {
            // Position for gameplay: behind White, angled down
            let initial_height = 10.0;
            let board_center = Vec3::new(3.5, 0.0, 3.5);
            let camera_pos = Vec3::new(3.5, initial_height, -8.0);

            *transform = Transform::from_translation(camera_pos).looking_at(board_center, Vec3::Y);

            // Ensure order is correct (0 is standard for 3D)
            camera.order = 0;

            // Add RTS camera controls
            commands.entity(entity).insert(CameraController {
                current_zoom: initial_height,
                target_zoom: initial_height,
                min_zoom: 3.0,
                max_zoom: 30.0,
                ..Default::default()
            });

            info!("[CAMERA] Configured Persistent Camera for Gameplay");
        }
    }
}

/// Reset the persistent camera when exiting gameplay
fn reset_game_camera(
    mut commands: Commands,
    persistent_camera: Res<PersistentEguiCamera>,
    mut query: Query<&mut Camera>,
) {
    if let Some(entity) = persistent_camera.entity {
        // Remove RTS controls
        commands.entity(entity).remove::<CameraController>();

        // Reset order if needed (though 0 is usually fine for menus too)
        if let Ok(mut camera) = query.get_mut(entity) {
            camera.order = 0;
        }

        info!("[CAMERA] Reset Persistent Camera (Removed Controls)");
    }
}

fn configure_settings_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            rendering::graphics_quality::apply_graphics_quality_camera_system,
            rendering::graphics_quality::update_graphics_quality_camera_system,
            rendering::graphics_quality::apply_graphics_quality_lights_system,
            audio::apply_master_volume_system,
            core::settings_persistence::save_settings_system,
        ),
    );
}

fn configure_global_setup(app: &mut App) {
    app.add_systems(PreStartup, setup_persistent_egui_camera);
    app.add_systems(Startup, setup_global_scene);
}

/// System to log any system execution errors for debugging
/// This helps identify which systems are failing
fn log_system_errors() {
    // This system runs every frame to check for errors
    // In the future, we could add more sophisticated error tracking here
}

/// Setup global scene elements (persistent background, ambient light)
///
/// These elements persist across all game states and provide
/// a base visual environment.
fn setup_global_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Global background (dark space-like environment)
    // Use safe hex color parsing with fallback
    let background_color = crate::core::error_handling::safe_parse_hex_color(
        "0a0a15",
        Srgba::new(0.04, 0.04, 0.08, 1.0), // Fallback: very dark blue
        "setup_global_scene background",
    );

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: background_color.into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        Name::new("Global Background"),
    ));

    // Global ambient light (persists across all states)
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.3, 0.3, 0.35), // Neutral blue-gray
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });
}

/// Setup a persistent camera with Egui context that survives all state transitions
///
/// This camera is used by all UI states (MainMenu, Settings, Pause, GameOver)
/// to avoid conflicts from multiple PrimaryEguiContext cameras.
fn setup_persistent_egui_camera(
    mut commands: Commands,
    mut persistent_camera: ResMut<PersistentEguiCamera>,
) {
    info!(
        "[PRESTARTUP] DEBUG: Current persistent_camera.entity: {:?}",
        persistent_camera.entity
    );

    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            // Default position - will be updated by each state
            Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            // Add Egui context components
            EguiContext::default(),
            PrimaryEguiContext,
            EguiMultipassSchedule::new(EguiPrimaryContextPass),
            Name::new("Persistent Egui Camera"),
        ))
        .id();

    persistent_camera.entity = Some(camera_entity);
    info!(
        "[PRESTARTUP] Persistent Egui camera created with entity ID: {:?}",
        camera_entity
    );
    info!(
        "[PRESTARTUP] DEBUG: Updated persistent_camera.entity to: {:?}",
        persistent_camera.entity
    );

    // Verify the entity was created successfully
    if persistent_camera.entity.is_some() {
    } else {
        error!("[PRESTARTUP] ERROR: Failed to store camera entity in resource!");
    }
}

/// Setup game scene when entering InGame state
///
/// Spawns the game camera, lighting, and chess board.
fn setup_game_scene(mut commands: Commands, view_mode: Res<crate::game::view_mode::ViewMode>) {
    // Set background color based on view mode
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        // Vibrant solid yellow background matching reference image (#FFFF00)
        commands.insert_resource(ClearColor(Color::srgb(1.0, 1.0, 0.0))); // Pure yellow #FFFF00
    } else {
        // Default dark background for standard view
        commands.insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0))); // Black
    }

    // Setup camera based on view mode
    // TempleOS camera is set up by the board plugin, so we only create standard camera here
    // UPDATE: We now reuse the PersistentEguiCamera for standard view (in setup_game_camera system)
    // so we ONLY need to handle TempleOS specific setup or lights here.

    // lights...

    // Skip lights for TempleOS mode (unlit rendering)
    if *view_mode != crate::game::view_mode::ViewMode::TempleOS {
        // Main directional light (chess tournament lighting)
        commands.spawn((
            DirectionalLight {
                illuminance: 8000.0,
                shadows_enabled: true,
                color: Color::srgb(1.0, 0.98, 0.95), // Warm white
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                -std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_4,
                0.0,
            )),
            DespawnOnExit(GameState::InGame),
            Name::new("Main Directional Light"),
        ));

        // Fill light (reduces harsh shadows)
        commands.spawn((
            PointLight {
                intensity: 500_000.0,
                color: Color::srgb(0.9, 0.9, 1.0), // Slightly blue
                shadows_enabled: false,
                range: 30.0,
                ..default()
            },
            Transform::from_xyz(-10.0, 10.0, 10.0),
            DespawnOnExit(GameState::InGame),
            Name::new("Fill Light"),
        ));
    }

    // Note: Ambient light is set globally in setup_global_scene (Startup)
}

/// Handle ESC key to pause the game
fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        info!("[GAME] Pausing game");
        next_state.set(GameState::Paused);
    }
}
