use crate::core::GameSettings;
use bevy::prelude::*;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use directories::ProjectDirs;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

#[cfg(not(target_arch = "wasm32"))]
const SETTINGS_FILENAME: &str = "settings.json";

#[cfg(not(target_arch = "wasm32"))]
fn get_settings_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        crate::core::paths::internal_data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(SETTINGS_FILENAME)
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Some(proj_dirs) = ProjectDirs::from("com", "trilltino", "XFChess") {
            let config_dir = proj_dirs.config_dir();
            config_dir.join(SETTINGS_FILENAME)
        } else {
            // Fallback to current directory
            PathBuf::from(SETTINGS_FILENAME)
        }
    }
}

pub fn load_settings_system(mut commands: Commands) {
    #[cfg(target_arch = "wasm32")]
    {
        match LocalStorage::get("xfchess_settings") {
            Ok(mut settings) => {
                // Sync colors from serialized format (if needed, dependent on how serde handles it)
                // Assuming serde handles the GameSettings struct cleanly, but if we need manual sync:
                let temp_settings: GameSettings = settings;
                // Note: GameSettings might need manual sync if dynamic lighting colors need it
                // But for now let's assume standard deserialization covers most
                info!("[SETTINGS] Loaded settings from LocalStorage");
                commands.insert_resource(temp_settings);
                return;
            }
            Err(_) => {
                info!("[SETTINGS] No settings found in LocalStorage, using defaults.");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let settings_path = get_settings_path();

        if settings_path.exists() {
            match fs::read_to_string(&settings_path) {
                Ok(contents) => {
                    match serde_json::from_str::<GameSettings>(&contents) {
                        Ok(settings) => {
                            // Sync colors from serialized format

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
            info!(
                "[SETTINGS] No settings file found at {:?}. Using defaults.",
                settings_path
            );
        }
    }

    // Use default settings if load failed
    commands.insert_resource(GameSettings::default());
}

pub fn save_settings_system(settings: ResMut<GameSettings>) {
    if !settings.is_changed() {
        return;
    }

    // Sync colors for serialization

    #[cfg(target_arch = "wasm32")]
    {
        match LocalStorage::set("xfchess_settings", settings.as_ref()) {
            Ok(_) => info!("[SETTINGS] Saved settings to LocalStorage"),
            Err(e) => error!(
                "[SETTINGS] Failed to save settings to LocalStorage: {:?}",
                e
            ),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let settings_path = get_settings_path();

        // Ensure the directory exists
        if let Some(parent) = settings_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    error!(
                        "[SETTINGS] Failed to create settings directory at {:?}: {}",
                        parent, e
                    );
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
                    error!(
                        "[SETTINGS] Failed to write settings file at {:?}: {}",
                        settings_path, e
                    );
                }
            },
            Err(e) => {
                error!("[SETTINGS] Failed to serialize settings: {}", e);
            }
        }
    }
}
