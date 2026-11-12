//! Stockfish computer opponent and asynchronous UCI scheduling.

pub mod resource;
pub mod systems;

// Re-export for convenience
pub use resource::{AiMode, ChessAIResource};
pub use systems::{AIPlugin, AIStatistics, PendingAIMove};
