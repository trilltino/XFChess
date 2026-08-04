//! System parameter groups for UI systems
//!
//! Provides convenient SystemParam types that group related resources together
//! for UI rendering, following the bevy_egui pattern of using SystemParams
//! for cleaner APIs.

pub mod game_ui;
pub mod main_menu;

pub use game_ui::GameUIParams;
pub use main_menu::MainMenuUIContext;
