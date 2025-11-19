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
use std::fs;
use std::path::PathBuf;

/// Settings file path (in project root for simplicity)
const SETTINGS_FILE: &str = "settings.json";

/// Load settings from file on startup
///
/// Attempts to load settings from `settings.json`. If the file doesn't exist or
/// is invalid, uses default settings. This system should run early in the startup
/// schedule to ensure settings are available for other systems.
///
/// # Errors
///
/// This function handles all errors internally and always succeeds, falling back
/// to default settings if loading fails. Errors are logged for debugging.
///
/// # Examples
///
/// ```rust,ignore
/// app.add_systems(Startup, load_settings_system.after(StartupSet::Startup));
/// ```
pub fn load_settings_system(mut commands: Commands) {
    let settings_path = PathBuf::from(SETTINGS_FILE);

    if settings_path.exists() {
        match fs::read_to_string(&settings_path) {
            Ok(contents) => {
                match serde_json::from_str::<GameSettings>(&contents) {
                    Ok(mut settings) => {
                        // Sync colors from serialized format
                        settings.dynamic_lighting.sync_from_serialized();
                        info!("[SETTINGS] Loaded settings from {}", SETTINGS_FILE);
                        commands.insert_resource(settings);
                        return;
                    }
                    Err(e) => {
                        warn!(
                            "[SETTINGS] Failed to parse settings file: {}. Using defaults.",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    "[SETTINGS] Failed to read settings file: {}. Using defaults.",
                    e
                );
            }
        }
    } else {
        info!("[SETTINGS] No settings file found. Using defaults.");
    }

    // Use default settings if load failed
    commands.insert_resource(GameSettings::default());
}

/// Save settings to file when they change
///
/// Watches for changes to [`GameSettings`] and automatically saves to `settings.json`.
/// This system runs in the Update schedule and only saves when settings have changed.
///
/// # Errors
///
/// Save errors are logged but don't interrupt gameplay. The system will attempt
/// to save again on the next change.
///
/// # Examples
///
/// ```rust,ignore
/// app.add_systems(Update, save_settings_system);
/// ```
pub fn save_settings_system(mut settings: ResMut<GameSettings>) {
    if !settings.is_changed() {
        return;
    }

    // Sync colors for serialization
    settings.dynamic_lighting.sync_for_serialization();

    let settings_path = PathBuf::from(SETTINGS_FILE);

    match serde_json::to_string_pretty(settings.as_ref()) {
        Ok(json) => match fs::write(&settings_path, json) {
            Ok(_) => {
                info!("[SETTINGS] Saved settings to {}", SETTINGS_FILE);
            }
            Err(e) => {
                error!("[SETTINGS] Failed to write settings file: {}", e);
            }
        },
        Err(e) => {
            error!("[SETTINGS] Failed to serialize settings: {}", e);
        }
    }
}
