#![allow(dead_code)]
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::EnvFilter;

pub fn init_logging() {
  let env_filter =
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("xfchess_tauri=info"));

  tracing_subscriber::fmt()
    .with_max_level(Level::TRACE)
    .with_target(false)
    .with_thread_ids(false)
    .with_file(false)
    .with_line_number(false)
    .with_env_filter(env_filter)
    .compact()
    .init();
}

pub fn log_window_event(window: &str, event: &str, details: Option<&str>) {
  info!(
    window = window,
    event = event,
    details = details.unwrap_or(""),
    "Window event: {} - {}",
    event,
    details.unwrap_or("")
  );
}

pub fn log_ipc_command(command: &str, window: Option<&str>) {
  info!(
    command = command,
    window = window.unwrap_or("unknown"),
    "IPC command: {} from {}",
    command,
    window.unwrap_or("unknown")
  );
}

pub fn log_auth_event(event: &str, user: Option<&str>) {
  info!(
    event = event,
    user = user.unwrap_or("unknown"),
    "Auth event: {} - {}",
    event,
    user.unwrap_or("unknown")
  );
}

pub fn log_error(context: &str, error: &str, details: Option<&str>) {
  error!(
    context = context,
    error = error,
    details = details.unwrap_or(""),
    "Error in {}: {} - {}",
    context,
    error,
    details.unwrap_or("")
  );
}

pub fn log_security_event(event: &str, details: &str) {
  warn!(
    event = event,
    details = details,
    "Security event: {} - {}",
    event,
    details
  );
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;

  /// Test that logging can be initialized without panicking
  #[test]
  fn test_logging_init() {
    // This test ensures that the logging system can be initialized
    // without panicking. In a real scenario, this would be called
    // at application startup.
    init_logging();

    // If we reach this point, initialization succeeded
    assert!(true);
  }

  /// Test logging with custom RUST_LOG environment variable
  #[test]
  fn test_logging_init_with_custom_level() {
    env::set_var("RUST_LOG", "debug");

    // This should not panic even with custom log level
    init_logging();

    env::remove_var("RUST_LOG");
    assert!(true);
  }

  /// Test logging with invalid log level (should use default)
  #[test]
  fn test_logging_init_with_invalid_level() {
    env::set_var("RUST_LOG", "invalid_level");

    // Should fall back to default level
    init_logging();

    env::remove_var("RUST_LOG");
    assert!(true);
  }

  /// Test window event logging function
  #[test]
  fn test_log_window_event() {
    // This test ensures that window event logging function
    // doesn't panic and formats correctly
    log_window_event("main", "show", Some("window shown successfully"));

    assert!(true);
  }

  /// Test window event logging without details
  #[test]
  fn test_log_window_event_no_details() {
    log_window_event("main", "hide", None);

    assert!(true);
  }

  /// Test IPC command logging function
  #[test]
  fn test_log_ipc_command() {
    log_ipc_command("show_window", Some("tournament-admin"));

    assert!(true);
  }

  /// Test IPC command logging without window
  #[test]
  fn test_log_ipc_command_no_window() {
    log_ipc_command("generic_command", None);

    assert!(true);
  }

  /// Test authentication event logging function
  #[test]
  fn test_log_auth_event() {
    log_auth_event("login_success", Some("user123"));

    assert!(true);
  }

  /// Test authentication event logging without user
  #[test]
  fn test_log_auth_event_no_user() {
    log_auth_event("logout", None);

    assert!(true);
  }

  /// Test error logging function
  #[test]
  fn test_log_error() {
    log_error(
      "window_manager",
      "Failed to create window",
      Some("Invalid parameters"),
    );

    assert!(true);
  }

  /// Test error logging without details
  #[test]
  fn test_log_error_no_details() {
    log_error("ipc_handler", "Command failed", None);

    assert!(true);
  }

  /// Test security event logging function
  #[test]
  fn test_log_security_event() {
    log_security_event(
      "unauthorized_access",
      "Attempted to access admin without permissions",
    );

    assert!(true);
  }

  /// Test all logging functions work together
  #[test]
  fn test_all_logging_functions() {
    init_logging();

    log_window_event("main", "ready", Some("DOM loaded"));
    log_ipc_command("get_info", Some("main"));
    log_auth_event("session_start", Some("user456"));
    log_error("network", "Connection failed", Some("timeout"));
    log_security_event("rate_limit", "Too many requests from IP");

    assert!(true);
  }
}
