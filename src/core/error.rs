//! Error types for core module
//!
//! Provides custom error types for core functionality including settings persistence,
//! resource initialization, and state management.

use thiserror::Error;

/// Errors that can occur in the core module
#[derive(Error, Debug)]
pub enum CoreError {
    /// Settings file I/O error
    #[error("Settings I/O error: {0}")]
    SettingsIo(#[from] std::io::Error),

    /// Settings serialization/deserialization error
    #[error("Settings serialization error: {0}")]
    SettingsSerialization(#[from] serde_json::Error),

    /// Resource initialization error
    #[error("Resource initialization failed: {message}")]
    ResourceInit { message: String },

    /// Window configuration error
    #[error("Window configuration error: {message}")]
    WindowConfig { message: String },
}

/// Result type alias for core operations
pub type CoreResult<T> = Result<T, CoreError>;
