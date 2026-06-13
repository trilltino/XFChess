//! Board rendering module
//!
//! Manages chess board creation and coordinate labeling.

pub mod board;
/// Floating board coordinate labels — only used by the TempleOS theme.
#[cfg(feature = "templeos")]
pub mod coordinates;

// Re-export all public items
pub use board::*;
#[cfg(feature = "templeos")]
pub mod templeos_ui;
