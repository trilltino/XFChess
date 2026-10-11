//! Loading `GameSettings` at startup and saving them whenever they change.

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

/// Where the settings file lives: the platform config dir by default; tests point
/// it at a temporary file.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone, Debug)]
pub struct SettingsFile(pub PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Default for SettingsFile {
    fn default() -> Self {
        Self(default_settings_path())
    }
}

/// Loads settings in `Startup` and saves them in `ApplySettings` on every change
/// after that (not on the load itself, and never on frames that didn't change them).
pub(crate) fn register(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<SettingsFile>();
    app.add_systems(Startup, load_settings_system).add_systems(
        Update,
        save_settings_system
            .in_set(super::schedule::ApplySettings)
            .run_if(resource_changed::<GameSettings>.and_then(not(resource_added::<GameSettings>))),
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn default_settings_path() -> PathBuf {
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

pub fn load_settings_system(
    mut commands: Commands,
    #[cfg(not(target_arch = "wasm32"))] file: Res<SettingsFile>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        match LocalStorage::get("xfchess_settings") {
            Ok(mut settings) => {
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
        let settings_path = file.0.clone();

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

    commands.insert_resource(GameSettings::default());
}

/// Writes the settings file. Registered to run only when `GameSettings` changed.
pub fn save_settings_system(
    settings: Res<GameSettings>,
    #[cfg(not(target_arch = "wasm32"))] file: Res<SettingsFile>,
) {
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
        let settings_path = file.0.clone();

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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::core::GraphicsQuality;

    /// A settings path in a fresh temporary directory, removed on drop.
    struct TempSettings(PathBuf);

    impl TempSettings {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "xfchess-settings-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            Self(dir.join(SETTINGS_FILENAME))
        }
    }

    impl Drop for TempSettings {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = fs::remove_dir_all(dir);
            }
        }
    }

    /// An app with settings registered exactly as the game does, after its first
    /// frame (Startup has loaded the settings).
    fn app(file: &TempSettings) -> App {
        let mut app = App::new();
        app.insert_resource(SettingsFile(file.0.clone()));
        register(&mut app);
        app.update();
        app
    }

    fn settings(app: &mut App) -> Mut<'_, GameSettings> {
        app.world_mut().resource_mut::<GameSettings>()
    }

    #[test]
    fn loading_defaults_does_not_write_a_file() {
        let file = TempSettings::new("load");
        let mut app = app(&file);
        app.update();
        assert!(!file.0.exists());
    }

    #[test]
    fn a_change_is_saved_once_and_loads_back_next_launch() {
        let file = TempSettings::new("roundtrip");
        let mut app = app(&file);
        settings(&mut app).master_volume = 0.3;
        settings(&mut app).graphics_quality = GraphicsQuality::Low;
        app.update();
        assert!(file.0.exists(), "a change is saved");

        fs::remove_file(&file.0).unwrap();
        app.update();
        app.update();
        assert!(!file.0.exists(), "frames without a change don't write");

        settings(&mut app).show_hints = false;
        app.update();
        let next_launch = self::app(&file);
        let loaded = next_launch.world().resource::<GameSettings>();
        assert_eq!(loaded.master_volume, 0.3);
        assert_eq!(loaded.graphics_quality, GraphicsQuality::Low);
        assert!(!loaded.show_hints);
    }

    /// The settings panel binds its widgets without change detection and only
    /// calls `set_changed` when a widget actually changed.
    #[test]
    fn widget_binding_without_an_edit_does_not_save() {
        let file = TempSettings::new("bypass");
        let mut app = app(&file);
        settings(&mut app).bypass_change_detection().master_volume = 0.9;
        app.update();
        assert!(!file.0.exists());

        settings(&mut app).set_changed();
        app.update();
        assert!(file.0.exists());
    }
}