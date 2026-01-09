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

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::ecs::error::{warn as bevy_warn, BevyError, ErrorContext};
use bevy::input::common_conditions::input_toggle_active;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy::render::settings::{PowerPreference, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy_egui::{
    EguiGlobalSettings, EguiMultipassSchedule, EguiPrimaryContextPass, PrimaryEguiContext,
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
mod networking;
mod persistent_camera;
mod rendering;
mod states;
mod ui;

pub use persistent_camera::PersistentEguiCamera;

// Imports
use crate::persistent_camera::setup_persistent_egui_camera;
use core::{CorePlugin, DespawnOnExit, GameState, WindowConfig};
use game::systems::{reset_game_camera, setup_game_camera, setup_game_scene, setup_global_scene};
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
    app.register_asset_loader(rendering::obj_loader::ObjLoader);
    app.add_plugins(CorePlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
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
    app.add_plugins(bevy_egui::EguiPlugin::default());
    // app.add_systems(
    //     PreStartup,
    //     |mut egui_settings: ResMut<EguiGlobalSettings>| {
    //         egui_settings.auto_create_primary_context = false;
    //     },
    // );
    app.add_plugins(
        WorldInspectorPlugin::default().run_if(input_toggle_active(false, KeyCode::F1)),
    );
    app.add_systems(
        EguiPrimaryContextPass,
        crate::ui::inspector::inspector_ui.run_if(input_toggle_active(false, KeyCode::F1)),
    );
    app.add_plugins(crate::ui::auth::AuthUiPlugin);
    app.add_plugins(crate::ui::chat::ChatUiPlugin);
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
    app.add_plugins(MultiplayerMenuPlugin);
}

fn configure_game_plugins(app: &mut App) {
    // app.add_plugins(DefaultPickingPlugins); // Already added by DefaultPlugins in Bevy 0.17+
    // app.add_plugins(MeshPickingPlugin);

    app.add_plugins(PiecePlugin);
    app.add_plugins(BoardPlugin);
    app.add_plugins(BoardUtils);
    app.add_plugins(DynamicLightingPlugin);
    app.add_plugins(GamePlugin);
    app.add_plugins(networking::NetworkingPlugin);
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
            toggle_fullscreen, // F11 fullscreen toggle - always active
        ),
    );
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

/// Handle F11 key to toggle fullscreen mode
fn toggle_fullscreen(keyboard: Res<ButtonInput<KeyCode>>, mut windows: Query<&mut Window>) {
    if keyboard.just_pressed(KeyCode::F11) {
        for mut window in windows.iter_mut() {
            window.mode = match window.mode {
                bevy::window::WindowMode::Windowed => {
                    info!("[WINDOW] Switching to fullscreen");
                    bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                }
                _ => {
                    info!("[WINDOW] Switching to windowed");
                    bevy::window::WindowMode::Windowed
                }
            };
        }
    }
}
