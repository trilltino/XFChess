//! Chess engine and performance resources
//!
//! Manages the chess engine integration and fast board state for performance optimization.

pub mod engine;
pub mod fast_board;

// Re-export all public items
pub use engine::*;
pub use fast_board::*;
