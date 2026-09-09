use std::fmt;

#[derive(Debug, Clone)]
pub enum AppError {
  Io(String),
  Config(String),
  Auth(String),
  Window(String),
  Ipc(String),
  Validation(String),
  External(String),
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

impl AppError {
  pub fn io<S: Into<String>>(msg: S) -> Self {
    AppError::Io(msg.into())
  }

  pub fn config<S: Into<String>>(msg: S) -> Self {
    AppError::Config(msg.into())
  }

  pub fn auth<S: Into<String>>(msg: S) -> Self {
    AppError::Auth(msg.into())
  }

  pub fn window<S: Into<String>>(msg: S) -> Self {
    AppError::Window(msg.into())
  }

  pub fn ipc<S: Into<String>>(msg: S) -> Self {
    AppError::Ipc(msg.into())
  }

  pub fn validation<S: Into<String>>(msg: S) -> Self {
    AppError::Validation(msg.into())
  }

  pub fn external<S: Into<String>>(msg: S) -> Self {
    AppError::External(msg.into())
  }

  pub fn generic<S: Into<String>>(msg: S) -> Self {
    AppError::Generic(msg.into())
  }
}

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
