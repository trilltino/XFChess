use std::convert::Infallible;

use h3::error::{ConnectionError, StreamError};
use iroh::{KeyParsingError, endpoint::ConnectError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request validation error: {0}")]
    RequestValidation(#[from] RequestValidationError),

    #[error("Response validation error: {0}")]
    ResponseValidation(#[from] ResponseValidationError),

    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("HTTP error: {0}")]
    Http(#[from] http::Error),

    #[error("Middleware error: {0}")]
    Middleware(#[from] MiddlewareError),

    #[error("{0}")]
    Shared(#[from] std::sync::Arc<Self>),

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RequestValidationError {
    #[error("Missing URI authority")]
    MissingAuthority,

    #[error("Bad peer ID: {0}")]
    BadPeerId(KeyParsingError),

    #[cfg(feature = "json")]
    #[error("JSON serialization error: {0}")]
    JsonSerialize(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ResponseValidationError {
    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[cfg(feature = "json")]
    #[error("JSON deserialization error: {0}")]
    JsonDeserialize(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connect failed: {0}")]
    Connect(#[from] ConnectError),

    #[error("QUIC/HTTP3 connection error: {0}")]
    Connection(#[from] ConnectionError),

    #[error("Stream error: {0}")]
    Stream(#[from] StreamError),
}

#[derive(Debug, thiserror::Error)]
pub enum MiddlewareError {
    #[error("Request timed out")]
    Timeout,

    #[error("Redirect limit exceeded")]
    RedirectLimitExceeded,

    #[error("Retry attempts exceeded, final error: {0}")]
    RetryLimitExceeded(Box<Error>),

    #[error("{0}")]
    Other(String),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
