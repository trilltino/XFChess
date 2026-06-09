//! Error types for chess engine
//!
//! Provides custom error types for chess engine operations including
//! move validation, game state queries, and engine initialization.

#[cfg(not(feature = "std"))]
use alloc::string::String;


#[cfg(feature = "std")]
use thiserror::Error;

/// Errors that can occur in the chess engine
#[derive(Debug)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum ChessEngineError {
    /// Invalid move attempted
    #[cfg_attr(feature = "std", error("Invalid move: from square {from} to square {to}"))]
    InvalidMove { from: i8, to: i8 },

    /// Invalid square index (out of bounds)
    #[cfg_attr(feature = "std", error("Invalid square index: {square} (must be 0-63)"))]
    InvalidSquare { square: i8 },

    /// No piece at source square
    #[cfg_attr(feature = "std", error("No piece at source square {square}"))]
    NoPieceAtSquare { square: i8 },

    /// Piece does not belong to the specified color
    #[cfg_attr(feature = "std", error("Piece at square {square} does not belong to color {color}"))]
    WrongPieceColor { square: i8, color: i64 },

    /// Engine state error
    #[cfg_attr(feature = "std", error("Engine state error: {message}"))]
    EngineState { message: String },

    /// Memory allocation error
    #[cfg_attr(feature = "std", error("Failed to allocate memory for transposition table"))]
    AllocationError,

    /// Search algorithm error - stack corruption or logic error
    #[cfg_attr(feature = "std", error("Search algorithm error: {message}"))]
    SearchError { message: String },

    /// Stack underflow in search algorithm
    #[cfg_attr(feature = "std", error("Stack underflow in search algorithm at depth {depth}"))]
    StackUnderflow { depth: i32 },

    /// Best move not set in search frame
    #[cfg_attr(feature = "std", error("Best move not set in search frame at depth {depth}"))]
    BestMoveNotSet { depth: i32 },
}

/// Result type alias for chess engine operations
pub type ChessEngineResult<T> = Result<T, ChessEngineError>;