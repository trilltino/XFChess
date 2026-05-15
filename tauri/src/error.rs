//! Error handling for XFChess Tauri application.
//!
//! This module provides custom error types for different domains
//! of the application to improve error handling and debugging.
//!
//! # Error Types
//!
//! - `AppError`: General application errors
//! - `IpcError`: IPC communication errors
//! - `WindowError`: Window management errors
//! - `ConfigError`: Configuration errors
//! - `AuthError`: Authentication errors

use std::fmt;

/// General application error type.
///
/// This enum represents common errors that can occur throughout
/// the XFChess application. It provides context-specific
/// error information for better debugging and user feedback.
#[derive(Debug, Clone)]
pub enum AppError {
  /// I/O related errors (file operations, network, etc.)
  Io(String),
  /// Configuration related errors
  Config(String),
  /// Authentication related errors
  Auth(String),
  /// Window management errors
  Window(String),
  /// IPC communication errors
  Ipc(String),
  /// Validation errors
  Validation(String),
  /// External service errors
  External(String),
  /// Generic errors with custom message
  Generic(String),
}

impl fmt::Display for AppError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      AppError::Io(msg) => write!(f, "I/O error: {}", msg),
      AppError::Config(msg) => write!(f, "Configuration error: {}", msg),
      AppError::Auth(msg) => write!(f, "Authentication error: {}", msg),
      AppError::Window(msg) => write!(f, "Window error: {}", msg),
      AppError::Ipc(msg) => write!(f, "IPC error: {}", msg),
      AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
      AppError::External(msg) => write!(f, "External service error: {}", msg),
      AppError::Generic(msg) => write!(f, "Error: {}", msg),
    }
  }
}

impl std::error::Error for AppError {}

/// Convenience constructors for common error types.
impl AppError {
  /// Create a new I/O error
  pub fn io<S: Into<String>>(msg: S) -> Self {
    AppError::Io(msg.into())
  }

  /// Create a new configuration error
  pub fn config<S: Into<String>>(msg: S) -> Self {
    AppError::Config(msg.into())
  }

  /// Create a new authentication error
  pub fn auth<S: Into<String>>(msg: S) -> Self {
    AppError::Auth(msg.into())
  }

  /// Create a new window error
  pub fn window<S: Into<String>>(msg: S) -> Self {
    AppError::Window(msg.into())
  }

  /// Create a new IPC error
  pub fn ipc<S: Into<String>>(msg: S) -> Self {
    AppError::Ipc(msg.into())
  }

  /// Create a new validation error
  pub fn validation<S: Into<String>>(msg: S) -> Self {
    AppError::Validation(msg.into())
  }

  /// Create a new external service error
  pub fn external<S: Into<String>>(msg: S) -> Self {
    AppError::External(msg.into())
  }

  /// Create a new generic error
  pub fn generic<S: Into<String>>(msg: S) -> Self {
    AppError::Generic(msg.into())
  }
}

/// Result type alias for application operations.
pub type AppResult<T> = Result<T, AppError>;

// Implement conversions from common error types
impl From<std::io::Error> for AppError {
  fn from(err: std::io::Error) -> Self {
    AppError::io(err.to_string())
  }
}

impl From<serde_json::Error> for AppError {
  fn from(err: serde_json::Error) -> Self {
    AppError::validation(format!("JSON serialization error: {}", err))
  }
}

impl From<tauri::Error> for AppError {
  fn from(err: tauri::Error) -> Self {
    AppError::ipc(format!("Tauri error: {}", err))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_app_error_display() {
    let error = AppError::io("Failed to read file");
    assert_eq!(error.to_string(), "I/O error: Failed to read file");

    let error = AppError::config("Invalid port");
    assert_eq!(error.to_string(), "Configuration error: Invalid port");

    let error = AppError::auth("Invalid credentials");
    assert_eq!(
      error.to_string(),
      "Authentication error: Invalid credentials"
    );
  }

  #[test]
  fn test_app_error_convenience_constructors() {
    let error = AppError::window("Window not found");
    match error {
      AppError::Window(msg) => assert_eq!(msg, "Window not found"),
      _ => panic!("Expected Window error variant"),
    }

    let error = AppError::validation("Invalid input");
    match error {
      AppError::Validation(msg) => assert_eq!(msg, "Invalid input"),
      _ => panic!("Expected Validation error variant"),
    }
  }

  #[test]
  fn test_error_conversions() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let app_err: AppError = io_err.into();
    match app_err {
      AppError::Io(msg) => assert!(msg.contains("file not found")),
      _ => panic!("Expected Io error variant"),
    }
  }

  #[test]
  fn test_app_result_type() {
    fn returns_success() -> AppResult<String> {
      Ok("success".to_string())
    }

    fn returns_error() -> AppResult<String> {
      Err(AppError::generic("something went wrong"))
    }

    assert!(returns_success().is_ok());
    assert!(returns_error().is_err());
  }
}
