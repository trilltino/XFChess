//! Window configuration resource
//!
//! Centralizes window settings for the application, allowing easy configuration
//! and modification of window properties.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, VideoModeSelection, Window};

/// Configuration for the primary application window
///
/// This resource stores window settings that can be modified before window creation
/// or used to configure the window after creation.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct WindowConfig {
    /// Window title
    pub title: String,
    /// Window width in logical pixels
    pub width: u32,
    /// Window height in logical pixels
    pub height: u32,
    /// Whether the window should be resizable
    pub resizable: bool,
    /// Whether the window should start maximized
    pub maximized: bool,
    /// Whether the window should start in fullscreen mode
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
            fullscreen: false,
        }
    }
}

impl WindowConfig {
    /// Create a Bevy Window from this configuration
    pub fn to_window(&self) -> Window {
        use bevy::window::WindowResolution;
        Window {
            title: self.title.clone(),
            resolution: WindowResolution::new(self.width, self.height),
            resizable: self.resizable,
            present_mode: PresentMode::AutoVsync,
            mode: if self.fullscreen {
                bevy::window::WindowMode::Fullscreen(
                    MonitorSelection::Current,
                    VideoModeSelection::Current,
                )
            } else {
                bevy::window::WindowMode::Windowed
            },
            ..default()
        }
    }

    /// Update window configuration from an existing window
    #[allow(dead_code)]
    pub fn from_window(window: &Window) -> Self {
        let resolution = &window.resolution;
        Self {
            title: window.title.clone(),
            width: resolution.width() as u32,
            height: resolution.height() as u32,
            resizable: window.resizable,
            maximized: false, // Window doesn't expose maximized state directly in Bevy 0.17
            fullscreen: matches!(
                window.mode,
                bevy::window::WindowMode::Fullscreen(..)
                    | bevy::window::WindowMode::BorderlessFullscreen(_)
            ),
        }
    }
}
