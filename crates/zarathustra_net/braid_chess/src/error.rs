//! Error types for braid_chess.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BraidChessError {
    #[error("Invalid Braid chess resource path: '{0}'")]
    InvalidPath(String),

    #[error("Unknown chess game resource: '{0}' (expected moves, clock, engine, or chat)")]
    UnknownResource(String),

    #[error("JSON serialisation error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("HTTP error status {0}")]
    HttpStatus(u16),
}
