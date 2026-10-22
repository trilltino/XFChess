//! App infrastructure every other plugin builds on: the [`GameState`] machine and
//! [`GameMode`] ([`state`]), frame ordering ([`schedule`]), [`settings`] and their
//! persistence, window and camera setup, [`diagnostics`], update checks
//! ([`update_check`]) and background I/O ([`net_task`]).
//! [`CorePlugin`] must be added before every other XFChess plugin.

#[cfg(target_os = "android")]
pub mod android_paths;
pub mod diagnostics;
pub mod egui_camera;
pub mod net_task;
pub mod plugin;
pub mod schedule;
pub mod settings;
pub mod state;
pub mod statistics;
pub mod update_check;
pub mod window;

pub use plugin::CorePlugin;
pub use settings::*;
pub use state::*;
pub use statistics::GameStatistics;
pub use window::WindowConfig;
