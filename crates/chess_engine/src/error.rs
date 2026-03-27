//! Error types for chess engine
//!
//! Provides custom error types for chess engine operations including
//! move validation, game state queries, and engine initialization.

use thiserror::Error;

/// Errors that can occur in the chess engine
#[derive(Error, Debug)]
pub enum ChessEngineError {
    /// Invalid move attempted
    #[error("Invalid move: from square {from} to square {to}")]
    InvalidMove { from: i8, to: i8 },

    /// Invalid square index (out of bounds)
    #[error("Invalid square index: {square} (must be 0-63)")]
    InvalidSquare { square: i8 },

    /// No piece at source square
    #[error("No piece at source square {square}")]
    NoPieceAtSquare { square: i8 },

    /// Piece does not belong to the specified color
    #[error("Piece at square {square} does not belong to color {color}")]
    WrongPieceColor { square: i8, color: i64 },

    /// Engine state error
    #[error("Engine state error: {message}")]
    EngineState { message: String },

    /// Memory allocation error
    #[error("Failed to allocate memory for transposition table")]
    AllocationError,

    /// Search algorithm error - stack corruption or logic error
    #[error("Search algorithm error: {message}")]
    SearchError { message: String },

    /// Stack underflow in search algorithm
    #[error("Stack underflow in search algorithm at depth {depth}")]
    StackUnderflow { depth: i32 },

    /// Best move not set in search frame
    #[error("Best move not set in search frame at depth {depth}")]
    BestMoveNotSet { depth: i32 },
}

/// Result type alias for chess engine operations
pub type ChessEngineResult<T> = Result<T, ChessEngineError>;
