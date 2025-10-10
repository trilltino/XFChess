//! UI module
//!
//! Handles all user interface systems:
//! - Launch menu
//! - In-game UI (egui)

pub mod launch_menu;
pub mod egui_systems;

// Re-export commonly used items
pub use launch_menu::*;
pub use egui_systems::*;
