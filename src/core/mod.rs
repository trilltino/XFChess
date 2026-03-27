//! Core module - Game state management and application infrastructure.

pub mod error_handling;
pub mod plugin;
pub mod resources;
pub mod settings_persistence;
pub mod state_lifecycle;
pub mod states;
pub mod window_config;

pub use plugin::CorePlugin;
pub use resources::*;
pub use states::*;
pub use window_config::WindowConfig;
