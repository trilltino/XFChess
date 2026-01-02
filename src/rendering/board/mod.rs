//! Board rendering module
//!
//! Manages chess board creation, theming, and coordinate labeling.

pub mod board;
pub mod board_theme;
pub mod coordinates;

// Re-export all public items
pub use board::*;
pub mod templeos_ui;
