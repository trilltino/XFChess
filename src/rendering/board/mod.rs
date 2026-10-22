//! The board: squares, coordinates, and the TempleOS theme.

#[cfg(feature = "templeos")]
pub mod coordinates;
pub mod spawn;

// Re-export all public items
pub use spawn::*;
#[cfg(feature = "templeos")]
pub mod templeos_ui;
