#![allow(dead_code)]
//! Configuration management for XFChess Tauri application.
//!
//! This module provides centralized access to environment variables
//! and configuration values used throughout the application.
//!
//! # Environment Variables
//!
//! - `SIGNING_SERVICE_URL`: Backend signing service URL
//! - `BACKEND_URL`: Alternative backend URL (fallback)
//! - `ADMIN_API_KEY`: API key for admin operations
//! - `XFCHESS_WALLET_PORT`: Port for wallet popup service
//! - `NODE_ENV`: Environment mode (development/production)
//! - `RUST_LOG`: Logging level configuration

/// Get the backend URL for API communication.
///
/// This function checks multiple environment variables in order of preference:
/// 1. `SIGNING_SERVICE_URL` - Specific signing service URL
/// 2. `BACKEND_URL` - General backend URL
/// 3. Default fallback to localhost:8090
///
/// # Returns
///
/// The backend URL as a String
///
/// # Examples
///
/// ```rust
/// let url = get_backend_url();
/// // Returns: "http://127.0.0.1:8090" (default)
/// ```
pub fn get_backend_url() -> String {
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string())
}

/// Get the admin API key for authenticated operations.
///
/// This function retrieves the admin API key from environment variables.
/// The key is optional and may not be set in all environments.
///
/// # Returns
///
/// - `Some(String)` if `ADMIN_API_KEY` is set
/// - `None` if the environment variable is not set
///
/// # Security Note
///
/// This key should be treated as sensitive and not logged or exposed.
pub fn get_admin_api_key() -> Option<String> {
  std::env::var("ADMIN_API_KEY").ok()
}

/// Get the port number for the wallet popup service.
///
/// This function reads the `XFCHESS_WALLET_PORT` environment variable
/// and falls back to 7454 if not set or invalid.
///
/// # Returns
///
/// The port number as u16
///
/// # Examples
///
/// ```rust
/// let port = get_wallet_port();
/// // Returns: 7454 (default) or configured port
/// ```
pub fn get_wallet_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

/// Determine if the application is running in development mode.
///
/// This function checks the `NODE_ENV` environment variable and also
/// falls back to Rust's debug_assertions configuration.
///
/// # Returns
///
/// `true` if in development mode, `false` otherwise
///
/// # Priority
///
/// 1. `NODE_ENV` environment variable set to "development"
/// 2. Rust's `debug_assertions` configuration
pub fn is_development() -> bool {
  std::env::var("NODE_ENV")
    .map(|env| env == "development")
    .unwrap_or(cfg!(debug_assertions))
}

/// Get the configured logging level.
///
/// This function retrieves the RUST_LOG environment variable
/// and falls back to "info" level if not set.
///
/// # Returns
///
/// The logging level as a String
///
/// # Examples
///
/// Common log levels: "error", "warn", "info", "debug", "trace"
pub fn get_log_level() -> String {
  std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;

  /// Test get_backend_url with SIGNING_SERVICE_URL set
  #[test]
  fn test_get_backend_url_with_signing_service() {
    env::set_var("SIGNING_SERVICE_URL", "https://signing.example.com");
    env::remove_var("BACKEND_URL");

    let url = get_backend_url();
    assert_eq!(url, "https://signing.example.com");

    env::remove_var("SIGNING_SERVICE_URL");
  }

  /// Test get_backend_url with BACKEND_URL set (fallback)
  #[test]
  fn test_get_backend_url_with_backend_url() {
    env::remove_var("SIGNING_SERVICE_URL");
    env::set_var("BACKEND_URL", "https://backend.example.com");

    let url = get_backend_url();
    assert_eq!(url, "https://backend.example.com");

    env::remove_var("BACKEND_URL");
  }

  /// Test get_backend_url with no environment variables (default)
  #[test]
  fn test_get_backend_url_default() {
    env::remove_var("SIGNING_SERVICE_URL");
    env::remove_var("BACKEND_URL");

    let url = get_backend_url();
    assert_eq!(url, "http://127.0.0.1:8090");
  }

  /// Test get_admin_api_key when set
  #[test]
  fn test_get_admin_api_key_set() {
    env::set_var("ADMIN_API_KEY", "test-api-key-123");

    let key = get_admin_api_key();
    assert_eq!(key, Some("test-api-key-123".to_string()));

    env::remove_var("ADMIN_API_KEY");
  }

  /// Test get_admin_api_key when not set
  #[test]
  fn test_get_admin_api_key_not_set() {
    env::remove_var("ADMIN_API_KEY");

    let key = get_admin_api_key();
    assert_eq!(key, None);
  }

  /// Test get_wallet_port with custom port
  #[test]
  fn test_get_wallet_port_custom() {
    env::set_var("XFCHESS_WALLET_PORT", "9000");

    let port = get_wallet_port();
    assert_eq!(port, 9000);

    env::remove_var("XFCHESS_WALLET_PORT");
  }

  /// Test get_wallet_port with invalid port (fallback to default)
  #[test]
  fn test_get_wallet_port_invalid() {
    env::set_var("XFCHESS_WALLET_PORT", "invalid");

    let port = get_wallet_port();
    assert_eq!(port, 7454); // Default fallback

    env::remove_var("XFCHESS_WALLET_PORT");
  }

  /// Test get_wallet_port with no environment variable (default)
  #[test]
  fn test_get_wallet_port_default() {
    env::remove_var("XFCHESS_WALLET_PORT");

    let port = get_wallet_port();
    assert_eq!(port, 7454);
  }

  /// Test is_development with NODE_ENV set to development
  #[test]
  fn test_is_development_true() {
    env::set_var("NODE_ENV", "development");

    let is_dev = is_development();
    assert!(is_dev);

    env::remove_var("NODE_ENV");
  }

  /// Test is_development with NODE_ENV set to production
  #[test]
  fn test_is_development_false() {
    env::set_var("NODE_ENV", "production");

    let is_dev = is_development();
    assert!(!is_dev);

    env::remove_var("NODE_ENV");
  }

  /// Test get_log_level with custom level
  #[test]
  fn test_get_log_level_custom() {
    env::set_var("RUST_LOG", "debug");

    let level = get_log_level();
    assert_eq!(level, "debug");

    env::remove_var("RUST_LOG");
  }

  /// Test get_log_level with no environment variable (default)
  #[test]
  fn test_get_log_level_default() {
    env::remove_var("RUST_LOG");

    let level = get_log_level();
    assert_eq!(level, "info");
  }
}
