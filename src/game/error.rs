//! Error types for game module
//!
//! Provides custom error types for game logic including move validation,
//! engine synchronization, and game state management.

/// Errors that can occur in game logic
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// Invalid move attempted
    #[error("Invalid move: {message}")]
    InvalidMove { message: String },

    /// Engine synchronization error
    #[error("Engine synchronization failed: {message}")]
    EngineSync { message: String },

    /// Piece not found at expected position
    #[error("Piece not found at position ({x}, {y})")]
    PieceNotFound { x: u8, y: u8 },

    /// Invalid game state transition
    #[error("Invalid game state transition: {message}")]
    InvalidStateTransition { message: String },

    /// Resource not initialized
    #[error("Required resource not initialized: {resource_name}")]
    ResourceNotInitialized { resource_name: String },
}

/// Result type alias for game operations
pub type GameResult<T> = Result<T, GameError>;
