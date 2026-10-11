//! App infrastructure every other plugin builds on: the [`GameState`] machine and
//! [`GameMode`], frame ordering ([`schedule`]), window setup, crash reporting,
//! settings persistence, update checks and background I/O ([`net_task`]).
//! [`CorePlugin`] must be added before every other XFChess plugin.

pub mod crash;
pub mod error_handling;
pub mod net_task;
#[cfg(target_os = "android")]
pub mod paths;
pub mod persistent_camera;
pub mod plugin;
pub mod resources;
pub mod schedule;
pub mod settings_persistence;
pub mod state_lifecycle;
pub mod states;
pub mod support_bundle;
pub mod updates;
pub mod window_config;

pub use plugin::CorePlugin;
pub use resources::*;
pub use states::*;
pub use window_config::WindowConfig;
