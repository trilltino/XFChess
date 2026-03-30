//! Board rendering module
//!
//! Manages chess board creation and coordinate labeling.

pub mod board;
pub mod coordinates;

// Re-export all public items
pub use board::*;
pub mod templeos_ui;
