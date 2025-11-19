//! Core plugin for XFChess
//!
//! Provides fundamental application setup including:
//! - Panic hook configuration for detailed crash reporting
//! - Window configuration
//! - Core resource initialization
//! - State management setup
//!
//! # Plugin Dependencies
//!
//! This plugin has no dependencies and should be added **first** before any other
//! XFChess plugins. It sets up foundational state management and resources that
//! other plugins depend on.
//!
//! # Plugin Order
//!
//! Recommended plugin order:
//! 1. [`CorePlugin`] - Foundation (state, resources)
//! 2. [`bevy::DefaultPlugins`] - Core Bevy functionality
//! 3. [`bevy_egui::EguiPlugin`] - UI framework
//! 4. [`crate::game::GamePlugin`] - Game logic
//! 5. State plugins (MainMenuPlugin, SettingsPlugin, etc.)
//! 6. Rendering plugins (PiecePlugin, BoardPlugin, etc.)
//!
//! This plugin should be added early in the plugin chain as it sets up
//! foundational systems and resources used throughout the application.

use bevy::prelude::*;
use std::panic;
use std::sync::{Mutex, OnceLock};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use super::{
    settings_persistence::load_settings_system,
    states::{log_game_state_system, validate_and_log_state_transitions},
    GameSettings, GameState, GameStatistics, InGameplay, InMenus, MenuState, PreviousState,
    StateLoggerTimer, StateTransitionTimer, WindowConfig,
};

/// Global state tracker for panic reporting
/// This allows the panic hook to report the current state even outside ECS context
static PANIC_STATE_TRACKER: OnceLock<Mutex<PanicStateInfo>> = OnceLock::new();

/// State information stored for panic reporting
#[derive(Debug, Clone, Default)]
struct PanicStateInfo {
    game_state: Option<GameState>,
    menu_state: Option<MenuState>,
}

/// Core plugin for XFChess application
///
/// Sets up fundamental application infrastructure including:
/// - Panic hook for detailed crash reporting
/// - Window configuration
/// - Core state management
/// - Settings persistence
///
/// # Usage
///
/// ```rust,ignore
/// App::new()
///     .add_plugins(CorePlugin)
///     .add_plugins(DefaultPlugins)
///     // ... other plugins
/// ```
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        // Initialize window configuration
        app.init_resource::<WindowConfig>();

        // Initialize core state management resources
        app.init_state::<GameState>()
            .add_sub_state::<MenuState>()
            .add_computed_state::<InMenus>()
            .add_computed_state::<InGameplay>()
            .init_resource::<PreviousState>()
            .init_resource::<StateTransitionTimer>()
            .init_resource::<StateLoggerTimer>();

        // Initialize core game resources
        // Note: GameSettings will be loaded from file in load_settings_system
        app.init_resource::<GameStatistics>();

        // Register types for reflection
        app.register_type::<WindowConfig>()
            .register_type::<PreviousState>()
            .register_type::<GameSettings>()
            .register_type::<GameStatistics>();

        // Add settings persistence system (runs in Startup schedule)
        app.add_systems(Startup, load_settings_system);

        // Add state logging and validation systems
        app.add_systems(
            Update,
            (
                log_game_state_system,
                validate_and_log_state_transitions,
                update_panic_state_tracker,
            ),
        );
    }

    fn finish(&self, _app: &mut App) {
        // Set up panic hook in finish() to ensure it's configured
        // after all plugins are built but before the app runs
        setup_panic_hook();

        // Enable full backtraces for debugging
        std::env::set_var("RUST_BACKTRACE", "full");
    }
}

/// Set up a custom panic hook that provides detailed crash information
///
/// This panic hook provides comprehensive information about panics including:
/// - Panic message
/// - Location (file, line, column)
/// - Current game state (GameState and MenuState)
/// - Full backtrace
///
/// The output is formatted for easy reading in PowerShell (ASCII-only, no box drawing).
/// Panic information is also written to a log file for later analysis.
fn setup_panic_hook() {
    // Initialize the state tracker
    PANIC_STATE_TRACKER.get_or_init(|| Mutex::new(PanicStateInfo::default()));

    panic::set_hook(Box::new(|panic_info| {
        // Collect panic information
        let panic_msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<unknown>".to_string()
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "<unknown>".to_string()
        };

        // Get state information
        let mut game_state_str = "<unknown>".to_string();
        let mut menu_state_str = "<not in menu>".to_string();
        if let Some(tracker) = PANIC_STATE_TRACKER.get() {
            if let Ok(state_info) = tracker.lock() {
                if let Some(game_state) = state_info.game_state {
                    game_state_str = format!("{:?}", game_state);
                }
                if let Some(menu_state) = state_info.menu_state {
                    menu_state_str = format!("{:?}", menu_state);
                }
            }
        }

        let backtrace = std::backtrace::Backtrace::capture();
        let backtrace_str = format!("{}", backtrace);

        // Format panic report
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let panic_report = format!(
            "PANIC DETECTED [{}]\n\
            ============================================\n\
            Message: {}\n\
            Location: {}\n\
            GameState: {}\n\
            MenuState: {}\n\
            \n\
            Backtrace:\n\
            {}\n\
            ============================================\n",
            timestamp, panic_msg, location, game_state_str, menu_state_str, backtrace_str
        );

        // Print to stderr (console)
        eprintln!("\n{}", panic_report);

        // Write to log file
        let logs_dir = Path::new("logs");
        if !logs_dir.exists() {
            let _ = fs::create_dir_all(logs_dir);
        }

        let log_file = logs_dir.join(format!("crash_{}.log", timestamp));
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&log_file)
        {
            let _ = writeln!(file, "{}", panic_report);
            eprintln!("[PANIC] Crash log written to: {:?}", log_file);
        }
    }));
}

/// System to update the panic state tracker with current state
/// This allows the panic hook to report the current state even outside ECS context
fn update_panic_state_tracker(
    game_state: Option<Res<State<GameState>>>,
    menu_state: Option<Res<State<MenuState>>>,
) {
    if let Some(tracker) = PANIC_STATE_TRACKER.get() {
        if let Ok(mut state_info) = tracker.lock() {
            if let Some(game_state_res) = game_state {
                state_info.game_state = Some(*game_state_res.get());
            }
            if let Some(menu_state_res) = menu_state {
                state_info.menu_state = Some(*menu_state_res.get());
            }
        }
    }
}
