#![allow(dead_code)]
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bs58;

pub fn encode_base64(data: &[u8]) -> String {
  B64.encode(data)
}

pub fn decode_base64(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
  B64.decode(data)
}

pub fn encode_base58(data: &[u8]) -> String {
  bs58::encode(data).into_string()
}

pub fn decode_base58(data: &str) -> Result<Vec<u8>, bs58::decode::Error> {
  bs58::decode(data).into_vec()
}

pub fn validate_token_format(token: &str) -> bool {
  // Basic validation: check if it looks like a valid token
  !token.is_empty() && token.len() >= 32 && token.len() <= 1024
}

pub fn hash_password(password: &str, salt: &str) -> String {
  use sha2::{Digest, Sha256};

  let mut hasher = Sha256::new();
  hasher.update(password.as_bytes());
  hasher.update(salt.as_bytes());
  format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_base64_encoding() {
    let data = b"hello world";
    let encoded = encode_base64(data);
    assert_eq!(encoded, "aGVsbG8gd29ybGQ=");

    let decoded = decode_base64(&encoded).unwrap();
    assert_eq!(decoded, data);
  }

  #[test]
  fn test_base58_encoding() {
    let data = b"hello world";
    let encoded = encode_base58(data);
    let decoded = decode_base58(&encoded).unwrap();
    assert_eq!(decoded, data);
  }

  #[test]
  fn test_token_validation() {
    assert!(validate_token_format("valid_token_12345"));
    assert!(!validate_token_format(""));
    assert!(!validate_token_format("short"));
    assert!(!validate_token_format(&"a".repeat(1025)));
  }
}
