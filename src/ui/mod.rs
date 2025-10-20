//! UI module - Egui-based user interfaces
//!
//! Manages all UI rendering using `bevy_egui`, providing both gameplay UI
//! and development/debugging interfaces:
//!
//! - **launch_menu**: Main menu for starting games and configuring settings
//! - **egui_systems**: In-game HUD showing timer, captured pieces, game state
//! - **inspector**: Debug UI for inspecting ECS state (F1 to toggle)
//!
//! # Bevy Egui Integration
//!
//! Uses `bevy_egui` (0.37.1) which provides:
//! - `EguiContexts` system parameter for accessing egui context
//! - Automatic input handling and rendering
//! - Integration with Bevy's window and input systems
//!
//! # Reference
//!
//! Egui patterns follow:
//! - `reference/bevy-inspector-egui/` - Inspector UI implementation
//! - `bevy_egui` examples - Context access and layout patterns
//!
//! The inspector integration is particularly useful for debugging entity hierarchies,
//! component values, and resource state during development.

pub mod launch_menu;
pub mod egui_systems;
pub mod game_ui;

// Re-export commonly used items
pub use launch_menu::*;
pub use game_ui::*;
