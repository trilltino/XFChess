//! Error types for braid_uri.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BraidUriError {
    #[error("Invalid Braid chess URI path: '{0}'")]
    InvalidPath(String),

    #[error("Unknown chess game resource: '{0}' (expected moves, clock, or engine)")]
    UnknownResource(String),

    #[error("JSON serialisation error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("HTTP error status {0}")]
    HttpStatus(u16),
}
