use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, Window};

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
                // Borderless (maximized window), not exclusive Fullscreen: exclusive
                // fullscreen owns the whole display surface, so nothing — including the
                // wallet popup — can render on top without minimizing the game first.
                bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            } else {
                bevy::window::WindowMode::Windowed
            },
            ..default()
        }
    }
}
