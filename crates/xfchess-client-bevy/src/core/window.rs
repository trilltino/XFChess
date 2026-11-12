use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, Window};

use super::{DisplayMode, GameSettings};

/// Apply saved display preferences to the game window without touching its size.
pub fn apply_display_settings(
    settings: Res<GameSettings>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let mode = match settings.display_mode {
        DisplayMode::BorderlessFullscreen => {
            bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        }
        DisplayMode::BorderlessWindowed | DisplayMode::Windowed => {
            bevy::window::WindowMode::Windowed
        }
    };
    let decorations = settings.display_mode == DisplayMode::Windowed;
    if window.mode != mode {
        window.mode = mode;
    }
    if window.decorations != decorations {
        window.decorations = decorations;
    }
}

#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub maximized: bool,
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "XFChess - Modern 3D Chess".to_string(),
            width: 1366,
            height: 768,
            resizable: true,
            maximized: false,
            fullscreen: true,
        }
    }
}

impl WindowConfig {
    pub fn to_window(&self) -> Window {
        use bevy::window::WindowResolution;
        Window {
            title: self.title.clone(),
            resolution: WindowResolution::new(self.width, self.height),
            resizable: self.resizable,
            present_mode: PresentMode::AutoVsync,
            mode: if self.fullscreen {
                // Borderless fullscreen lets wallet popups appear above the game.
                bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            } else {
                bevy::window::WindowMode::Windowed
            },
            ..default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::{PrimaryWindow, WindowMode};

    #[test]
    fn fullscreen_shortcut_restores_the_window_style_and_only_changes_the_game_window() {
        for starting_mode in [DisplayMode::Windowed, DisplayMode::BorderlessWindowed] {
            let mut app = App::new();
            app.insert_resource(GameSettings {
                display_mode: starting_mode,
                ..default()
            });
            let primary = app
                .world_mut()
                .spawn((Window::default(), PrimaryWindow))
                .id();
            let other = app.world_mut().spawn(Window::default()).id();
            app.add_systems(Update, apply_display_settings);
            app.update();
            let original_size = app
                .world()
                .get::<Window>(primary)
                .unwrap()
                .resolution
                .clone();

            app.add_systems(
                Update,
                crate::input::board::toggle_fullscreen.before(apply_display_settings),
            );
            app.update();
            assert!(matches!(
                app.world().get::<Window>(primary).unwrap().mode,
                WindowMode::BorderlessFullscreen(_)
            ));
            assert_eq!(
                app.world().resource::<GameSettings>().display_mode,
                DisplayMode::BorderlessFullscreen
            );

            app.update();
            let window = app.world().get::<Window>(primary).unwrap();
            assert_eq!(window.mode, WindowMode::Windowed);
            assert_eq!(window.decorations, starting_mode == DisplayMode::Windowed);
            assert_eq!(window.resolution, original_size);
            assert_eq!(
                app.world().resource::<GameSettings>().display_mode,
                starting_mode
            );
            let other_window = app.world().get::<Window>(other).unwrap();
            assert_eq!(other_window.mode, WindowMode::Windowed);
            assert!(other_window.decorations);
        }
    }

    #[test]
    fn settings_saved_before_display_modes_were_added_still_load() {
        let mut saved = serde_json::to_value(GameSettings::default()).unwrap();
        saved.as_object_mut().unwrap().remove("display_mode");
        let loaded: GameSettings = serde_json::from_value(saved).unwrap();
        assert_eq!(loaded.display_mode, DisplayMode::Windowed);
    }
}
