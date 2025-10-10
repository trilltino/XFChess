//! Chess game logic module
//!
//! This module contains all the core chess game logic including:
//! - Game state management (components and resources)
//! - Move validation (pure chess rules)
//! - System logic (ECS systems for gameplay)
//! - Plugin registration

pub mod components;
pub mod resources;
pub mod systems;
pub mod rules;
pub mod plugin;

// Re-export the plugin (main entry point)
pub use plugin::GamePlugin;
