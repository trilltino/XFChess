#![allow(dead_code)]

pub fn get_backend_url() -> String {
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string())
}

pub fn get_admin_api_key() -> Option<String> {
  std::env::var("ADMIN_API_KEY").ok()
}

pub fn get_wallet_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

pub fn is_development() -> bool {
  std::env::var("NODE_ENV")
    .map(|env| env == "development")
    .unwrap_or(cfg!(debug_assertions))
}

pub fn get_log_level() -> String {
  std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::env;

  #[test]
  fn test_get_backend_url_with_signing_service() {
    env::set_var("SIGNING_SERVICE_URL", "https://signing.example.com");
    env::remove_var("BACKEND_URL");

    let url = get_backend_url();
    assert_eq!(url, "https://signing.example.com");

    env::remove_var("SIGNING_SERVICE_URL");
  }

  #[test]
  fn test_get_backend_url_with_backend_url() {
    env::remove_var("SIGNING_SERVICE_URL");
    env::set_var("BACKEND_URL", "https://backend.example.com");

    let url = get_backend_url();
    assert_eq!(url, "https://backend.example.com");

    env::remove_var("BACKEND_URL");
  }

  #[test]
  fn test_get_backend_url_default() {
    env::remove_var("SIGNING_SERVICE_URL");
    env::remove_var("BACKEND_URL");

    let url = get_backend_url();
    assert_eq!(url, "http://127.0.0.1:8090");
  }

  #[test]
  fn test_get_admin_api_key_set() {
    env::set_var("ADMIN_API_KEY", "test-api-key-123");

    let key = get_admin_api_key();
    assert_eq!(key, Some("test-api-key-123".to_string()));

    env::remove_var("ADMIN_API_KEY");
  }

  #[test]
  fn test_get_admin_api_key_not_set() {
    env::remove_var("ADMIN_API_KEY");

    let key = get_admin_api_key();
    assert_eq!(key, None);
  }

  #[test]
  fn test_get_wallet_port_custom() {
    env::set_var("XFCHESS_WALLET_PORT", "9000");

    let port = get_wallet_port();
    assert_eq!(port, 9000);

    env::remove_var("XFCHESS_WALLET_PORT");
  }

  #[test]
  fn test_get_wallet_port_invalid() {
    env::set_var("XFCHESS_WALLET_PORT", "invalid");

    let port = get_wallet_port();
    assert_eq!(port, 7454); // Default fallback

    env::remove_var("XFCHESS_WALLET_PORT");
  }

  #[test]
  fn test_get_wallet_port_default() {
    env::remove_var("XFCHESS_WALLET_PORT");

    let port = get_wallet_port();
    assert_eq!(port, 7454);
  }

  #[test]
  fn test_is_development_true() {
    env::set_var("NODE_ENV", "development");

    let is_dev = is_development();
    assert!(is_dev);

    env::remove_var("NODE_ENV");
  }

  #[test]
  fn test_is_development_false() {
    env::set_var("NODE_ENV", "production");

    let is_dev = is_development();
    assert!(!is_dev);

    env::remove_var("NODE_ENV");
  }

  #[test]
  fn test_get_log_level_custom() {
    env::set_var("RUST_LOG", "debug");

    let level = get_log_level();
    assert_eq!(level, "debug");

    env::remove_var("RUST_LOG");
  }

  #[test]
  fn test_get_log_level_default() {
    env::remove_var("RUST_LOG");

    let level = get_log_level();
    assert_eq!(level, "info");
  }
}
