//! The computer opponent: the in-process `nimzovich` engine or an external Stockfish
//! process (desktop only), run off the main thread with results applied as moves.

pub mod resource;
pub mod systems;

// Re-export for convenience
pub use resource::{AiMode, ChessAIResource};
pub use systems::{AIPlugin, AIStatistics, PendingAIMove};
