#![allow(dead_code)]
use regex::Regex;
use url::Url;

pub fn validate_url(url_str: &str) -> Result<(), String> {
  match Url::parse(url_str) {
    Ok(_) => Ok(()),
    Err(e) => Err(format!("Invalid URL: {}", e)),
  }
}

pub fn validate_backend_url(url_str: &str) -> Result<(), String> {
  validate_url(url_str)?;

  let url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

  if url.scheme() != "http" && url.scheme() != "https" {
    return Err("Backend URL must use HTTP or HTTPS protocol".to_string());
  }

  if url.host().is_none() {
    return Err("Backend URL must include a valid host".to_string());
  }

  Ok(())
}

pub fn validate_token(token: &str) -> Result<(), String> {
  if token.is_empty() {
    return Err("Token cannot be empty".to_string());
  }

  if token.len() < 32 {
    return Err("Token must be at least 32 characters long".to_string());
  }

  if token.len() > 1024 {
    return Err("Token cannot exceed 1024 characters".to_string());
  }

  // Check for common invalid characters
  let invalid_chars = ['\0', '\r', '\n', '\t'];
  for &char in &invalid_chars {
    if token.contains(char) {
      return Err("Token contains invalid characters".to_string());
    }
  }

  Ok(())
}

pub fn validate_tournament_id(id: u64) -> Result<(), String> {
  if id == 0 {
    return Err("Tournament ID cannot be zero".to_string());
  }

  if id > u64::MAX / 2 {
    return Err("Tournament ID is too large".to_string());
  }

  Ok(())
}

pub fn validate_port(port: u16) -> Result<(), String> {
  if port == 0 {
    return Err("Port cannot be zero".to_string());
  }

  if port < 1024 {
    return Err("Port must be 1024 or higher".to_string());
  }

  if port > 65535 {
    return Err("Port cannot exceed 65535".to_string());
  }

  Ok(())
}

pub fn sanitize_string(input: &str) -> String {
  input
    .chars()
    .filter(|c| c.is_ascii() && !c.is_control())
    .collect::<String>()
    .trim()
    .to_string()
}

pub fn validate_email(email: &str) -> Result<(), String> {
  let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
    .map_err(|e| format!("Invalid email regex: {}", e))?;

  if !email_regex.is_match(email) {
    return Err("Invalid email format".to_string());
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_url_validation() {
    assert!(validate_url("https://example.com").is_ok());
    assert!(validate_url("http://localhost:8090").is_ok());
    assert!(validate_url("invalid-url").is_err());
    assert!(validate_url("").is_err());
  }

  #[test]
  fn test_backend_url_validation() {
    assert!(validate_backend_url("https://api.xfchess.com").is_ok());
    assert!(validate_backend_url("http://localhost:8090").is_ok());
    assert!(validate_backend_url("ftp://example.com").is_err());
    assert!(validate_backend_url("https://").is_err());
  }

  #[test]
  fn test_token_validation() {
    assert!(validate_token("valid_token_12345").is_ok());
    assert!(validate_token("").is_err());
    assert!(validate_token("short").is_err());
    assert!(validate_token(&"a".repeat(1025)).is_err());
    assert!(validate_token("invalid\ntoken").is_err());
  }

  #[test]
  fn test_tournament_id_validation() {
    assert!(validate_tournament_id(1).is_ok());
    assert!(validate_tournament_id(1000).is_ok());
    assert!(validate_tournament_id(0).is_err());
    assert!(validate_tournament_id(u64::MAX).is_err());
  }

  #[test]
  fn test_port_validation() {
    assert!(validate_port(8080).is_ok());
    assert!(validate_port(8090).is_ok());
    assert!(validate_port(0).is_err());
    assert!(validate_port(1023).is_err());
    assert!(validate_port(65536).is_err());
  }

  #[test]
  fn test_sanitization() {
    let input = "Hello\0World\r\nTest\tData";
    let sanitized = sanitize_string(input);
    assert_eq!(sanitized, "HelloWorldTestData");
  }

  #[test]
  fn test_email_validation() {
    assert!(validate_email("user@example.com").is_ok());
    assert!(validate_email("test.email+tag@domain.co.uk").is_ok());
    assert!(validate_email("invalid-email").is_err());
    assert!(validate_email("@domain.com").is_err());
    assert!(validate_email("user@").is_err());
  }
}
