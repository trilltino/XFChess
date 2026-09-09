pub mod resource;
pub mod systems;

// Re-export for convenience
pub use resource::{ChessAIResource, GameMode};
pub use systems::{AIPlugin, AIStatistics, PendingAIMove};
