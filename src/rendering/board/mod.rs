//! The board: squares, coordinates, and the TempleOS theme.

pub mod board;
#[cfg(feature = "templeos")]
pub mod coordinates;

// Re-export all public items
pub use board::*;
#[cfg(feature = "templeos")]
pub mod templeos_ui;
