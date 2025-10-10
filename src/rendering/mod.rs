//! Rendering module
//!
//! Handles all visual rendering for the chess game:
//! - Board rendering and setup
//! - Piece models and materials
//! - Rendering utilities

pub mod board;
pub mod pieces;
pub mod utils;

// Re-export commonly used items
pub use board::*;
pub use pieces::*;
pub use utils::*;
