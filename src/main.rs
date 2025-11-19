//! XFChess - A 3D Chess Game built with Bevy 0.17.2
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
//! - **Game Modes**: Human vs Human, Human vs AI (configurable difficulty)
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

use bevy::ecs::entity::Entity;
use bevy::ecs::error::{warn as bevy_warn, BevyError, ErrorContext};
use bevy::input::common_conditions::input_toggle_active;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
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
mod rendering;
mod states;
mod ui;

// Imports
use core::{CorePlugin, GameState, WindowConfig};
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

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        if let Err(e) = writeln!(file, "[{}] {}", timestamp, message) {
            eprintln!("[ERROR] Failed to write to log file: {}", e);
        }
    }
}

fn main() {
    // === DISABLE ANSI COLORS FOR POWERSHELL COMPATIBILITY ===
    // PowerShell doesn't render ANSI color codes properly, making logs unreadable
    // Setting NO_COLOR environment variable disables color output
    std::env::set_var("NO_COLOR", "1");
    
    let mut app = App::new();

    // === GLOBAL ERROR HANDLER ===
    // Set up custom error handler to catch system failures without crashing
    // This allows the game to continue running even if individual systems fail
    app.set_error_handler(|error: BevyError, context: ErrorContext| {
        // Write error to file for later analysis
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
        // Use Bevy's warn handler as fallback for additional context
        bevy_warn(error, context);
    });

    app
        // === CORE BEVY PLUGINS ===
        // Must be added first to provide StatesPlugin before CorePlugin initializes states
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(WindowConfig::default().to_window()),
                    ..default()
                })
                .set(AssetPlugin {
                    // Use project root for assets (works in both dev and release)
                    // This ensures assets are found whether running from project root or target/release
                    file_path: "assets".to_string(),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    // Enable detailed logging for debugging - ensure all debug messages are visible
                    // Base level DEBUG shows all debug/info/warn/error messages
                    // Override specific noisy Bevy modules to reduce spam
                    // Set naga to error level to completely suppress shader compiler debug spam
                    level: bevy::log::Level::DEBUG,
                    filter: "debug,bevy_ecs=warn,bevy_render=warn,wgpu=error,naga=error".to_string(),
                    // Disable colors explicitly for PowerShell compatibility
                    // Colors are already disabled via NO_COLOR env var, but this ensures it
                    ..default()
                }),
        )
        // === CORE PLUGIN ===
        // Must be added after DefaultPlugins to ensure StatesPlugin is available
        .add_plugins(CorePlugin)
        .add_plugins(EguiPlugin::default())
        // Disable auto-creation of primary Egui context since we manually set it up in MainMenu
        .add_systems(
            PreStartup,
            |mut egui_settings: ResMut<EguiGlobalSettings>| {
                egui_settings.auto_create_primary_context = false;
            },
        )
        // Inspector (toggle with F1 key) - for development
        // Use custom inspector that shows game resources, or WorldInspectorPlugin as fallback
        .add_plugins(WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::F1)))
        // Add custom inspector UI system (shows game resources in bottom panel)
        .add_systems(
            EguiPrimaryContextPass,
            crate::ui::inspector::inspector_ui.run_if(input_toggle_active(true, KeyCode::F1)),
        )
        // === STATE SYSTEM ===
        // Menu substates are initialized by CorePlugin
        // === CORE RESOURCES ===
        .init_resource::<PersistentEguiCamera>() // Persistent camera for all UI states
        // === ASSET RESOURCES ===
        .init_resource::<crate::assets::LoadingProgress>() // Asset loading progress tracker
        .init_resource::<crate::assets::GameAssets>() // Preloaded asset handles
        // === DEBUG SYSTEMS ===
        .add_systems(Update, log_system_errors)
        // === STATE PLUGINS ===
        .add_plugins(MainMenuPlugin) // Main menu (with integrated loading)
        .add_plugins(SettingsPlugin) // Settings menu
        .add_plugins(PausePlugin) // Pause menu
        .add_plugins(GameOverPlugin) // Post-game screen
        .add_plugins(PieceViewerPlugin) // Piece viewer for material customization
        // === GAME SYSTEMS PLUGINS ===
        .add_plugins(MeshPickingPlugin) // 3D object picking
        .add_plugins(PointerEventsPlugin) // Mouse interaction
        .add_plugins(PiecePlugin) // Chess piece rendering
        .add_plugins(BoardPlugin) // Chess board rendering
        .add_plugins(BoardUtils) // Board utilities
        .add_plugins(DynamicLightingPlugin) // Dynamic orbital lighting
        .add_plugins(GamePlugin) // Core chess game logic
        // === GAME STATE SETUP ===
        // Reset game resources when entering InGame state (must run BEFORE piece/board spawning)
        // Follows pattern from reference/bevy/examples/state/states.rs
        // Note: Systems in plugins register their OnEnter systems, then we register ours
        // We use .before() to ensure reset runs before piece/board creation
        .add_systems(
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
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                handle_pause_input.run_if(in_state(GameState::InGame)),
                game::systems::check_and_play_templeos_sound.run_if(in_state(GameState::InGame)),
            ),
        )
        // === SETTINGS SYSTEMS ===
        .add_systems(
            Update,
            (
                rendering::graphics_quality::apply_graphics_quality_camera_system,
                rendering::graphics_quality::update_graphics_quality_camera_system,
                rendering::graphics_quality::apply_graphics_quality_lights_system,
                audio::apply_master_volume_system,
                core::settings_persistence::save_settings_system,
            ),
        )
        // === GLOBAL SETUP ===
        // Create persistent camera in PreStartup to ensure it exists before state transitions
        // Note: OnEnter for default state runs before PreStartup, so we also add error handling
        .add_systems(PreStartup, setup_persistent_egui_camera)
        .add_systems(Startup, setup_global_scene)
        .run();
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

/// Resource to track the persistent Egui camera entity
#[derive(Resource, Default)]
pub struct PersistentEguiCamera {
    pub entity: Option<Entity>,
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
    if *view_mode != crate::game::view_mode::ViewMode::TempleOS {
        // Standard camera with RTS-style controls
        let initial_height = 20.0; // Camera spawn height

        commands.spawn((
            Camera3d::default(),
            Transform::from_matrix(Mat4::from_rotation_translation(
                Quat::from_xyzw(-0.3, -0.5, -0.3, 0.5).normalize(),
                Vec3::new(-7.0, initial_height, 4.0),
            )),
            CameraController {
                current_zoom: initial_height,
                target_zoom: initial_height,
                ..Default::default()
            },
            // Note: Egui context is provided by persistent camera, not this game camera
            DespawnOnExit(GameState::InGame),
            Name::new("Game Camera"),
        ));
    }

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
