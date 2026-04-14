//! System parameter groups for UI systems
//!
//! Provides convenient SystemParam types that group related resources together
//! for UI rendering, following the bevy_egui pattern of using SystemParams
//! for cleaner APIs.

pub mod main_menu;
pub mod game_ui;

pub use main_menu::MainMenuUIContext;
pub use game_ui::GameUIParams;
