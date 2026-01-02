//! Settings persistence system
//!
//! Saves and loads [`GameSettings`] to/from a JSON file. Provides automatic
//! persistence of user preferences across application sessions.
//!
//! # File Location
//!
//! Settings are stored in `settings.json` in the project root directory.
//! This location is chosen for simplicity and easy access during development.
//!
//! # Error Handling
//!
//! Both load and save operations handle errors gracefully:
//! - Load failures fall back to default settings
//! - Save failures are logged but don't interrupt gameplay
//!
//! # Usage
//!
//! Settings are automatically loaded on startup via [`load_settings_system`]
//! and saved automatically when changed via [`save_settings_system`].

use crate::core::GameSettings;
use bevy::prelude::*;
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

/// Settings filename
const SETTINGS_FILENAME: &str = "settings.json";

/// Helper to resolve the settings file path
///
/// Returns a path to `settings.json` in the user's configuration directory.
/// E.g., C:\Users\User\AppData\Roaming\trilltino\XFChess\settings.json
/// Falls back to local "settings.json" if the system config dir cannot be found.
fn get_settings_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "trilltino", "XFChess") {
        let config_dir = proj_dirs.config_dir();
        config_dir.join(SETTINGS_FILENAME)
    } else {
        // Fallback to current directory
        PathBuf::from(SETTINGS_FILENAME)
    }
}

/// Load settings from file on startup
///
/// Attempts to load settings from the system config directory. If the file doesn't exist or
/// is invalid, uses default settings. This system should run early in the startup
/// schedule to ensure settings are available for other systems.
pub fn load_settings_system(mut commands: Commands) {
    let settings_path = get_settings_path();

    if settings_path.exists() {
        match fs::read_to_string(&settings_path) {
            Ok(contents) => {
                match serde_json::from_str::<GameSettings>(&contents) {
                    Ok(mut settings) => {
                        // Sync colors from serialized format
                        settings.dynamic_lighting.sync_from_serialized();
                        info!("[SETTINGS] Loaded settings from {:?}", settings_path);
                        commands.insert_resource(settings);
                        return;
                    }
                    Err(e) => {
                        warn!(
                            "[SETTINGS] Failed to parse settings file at {:?}: {}. Using defaults.",
                            settings_path, e
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    "[SETTINGS] Failed to read settings file at {:?}: {}. Using defaults.",
                    settings_path, e
                );
            }
        }
    } else {
        info!("[SETTINGS] No settings file found at {:?}. Using defaults.", settings_path);
    }

    // Use default settings if load failed
    commands.insert_resource(GameSettings::default());
}

/// Save settings to file when they change
///
/// Watches for changes to [`GameSettings`] and automatically saves to `settings.json`
/// in the user's configuration directory.
pub fn save_settings_system(mut settings: ResMut<GameSettings>) {
    if !settings.is_changed() {
        return;
    }

    // Sync colors for serialization
    settings.dynamic_lighting.sync_for_serialization();

    let settings_path = get_settings_path();

    // Ensure the directory exists
    if let Some(parent) = settings_path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                error!("[SETTINGS] Failed to create settings directory at {:?}: {}", parent, e);
                return;
            }
        }
    }

    match serde_json::to_string_pretty(settings.as_ref()) {
        Ok(json) => match fs::write(&settings_path, json) {
            Ok(_) => {
                info!("[SETTINGS] Saved settings to {:?}", settings_path);
            }
            Err(e) => {
                error!("[SETTINGS] Failed to write settings file at {:?}: {}", settings_path, e);
            }
        },
        Err(e) => {
            error!("[SETTINGS] Failed to serialize settings: {}", e);
        }
    }
}
