//! JWT authentication module for the XFChess signing service.
//!
//! This module provides JWT token issuance and verification for wallet-based authentication.
//! Tokens are used to authorize API requests for session management and game operations.

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Default token time-to-live in seconds (7 days) when `JWT_TTL_SECS` is unset.
const DEFAULT_TOKEN_TTL_SECS: i64 = 604_800;

/// Resolves the token TTL from the `JWT_TTL_SECS` env var, falling back to the
/// default. Lets operators shorten the takeover window without a recompile.
fn token_ttl_secs() -> i64 {
    std::env::var("JWT_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_TOKEN_TTL_SECS)
}

/// JWT claims structure containing wallet identity and expiration.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Wallet public key (base58 encoded)
    pub sub: String,
    /// Issued-at timestamp (Unix epoch)
    #[serde(default)]
    pub iat: i64,
    /// Expiration timestamp (Unix epoch)
    pub exp: i64,
}

/// JWT issuer that can create and verify authentication tokens.
pub struct JwtIssuer {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtIssuer {
    /// Creates a new JwtIssuer with the provided secret key.
    ///
    /// # Arguments
    /// * `secret` - The secret key used for signing and verifying tokens
    pub fn new(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    /// Issues a JWT token for the given wallet public key.
    ///
    /// # Arguments
    /// * `wallet_pubkey` - The wallet's public key (base58 encoded)
    ///
    /// # Returns
    /// A signed JWT token string
    pub fn issue(&self, wallet_pubkey: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: wallet_pubkey.to_string(),
            iat: now,
            exp: now + token_ttl_secs(),
        };
        encode(&Header::default(), &claims, &self.encoding)
    }

    /// Verifies a JWT token and extracts the claims.
    ///
    /// # Arguments
    /// * `token` - The JWT token string to verify
    ///
    /// # Returns
    /// The decoded claims if the token is valid and not expired
    pub fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(token, &self.decoding, &Validation::default())?;
        Ok(data.claims)
    }
}

/// Request extension inserted by the dual-accept auth middleware when a request
/// authenticated via a per-user JWT (as opposed to the legacy relay secret).
/// Handlers can extract `Option<Extension<AuthedWallet>>` to apply per-caller
/// authorization (e.g. you may only open a session for your own wallet).
#[derive(Clone, Debug)]
pub struct AuthedWallet(pub String);

/// Extracts the Bearer token from an Authorization header value.
///
/// # Arguments
/// * `header` - The Authorization header string (e.g., "Bearer <token>")
///
/// # Returns
/// The token string if the header is properly formatted, None otherwise
pub fn extract_bearer(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_issue_and_verify_roundtrip() {
        let issuer = JwtIssuer::new("test_secret_123");
        let token = issuer.issue("wallet123").expect("issue should succeed");
        let claims = issuer.verify(&token).expect("verify should succeed");
        assert_eq!(claims.sub, "wallet123");
        assert!(claims.exp > Utc::now().timestamp());
    }

    #[test]
    fn jwt_verify_fails_with_bad_secret() {
        let issuer = JwtIssuer::new("correct_secret");
        let token = issuer.issue("wallet123").unwrap();
        let bad_issuer = JwtIssuer::new("wrong_secret");
        assert!(bad_issuer.verify(&token).is_err());
    }

    #[test]
    fn extract_bearer_valid() {
        assert_eq!(extract_bearer("Bearer abc123"), Some("abc123"));
    }

    #[test]
    fn extract_bearer_missing_prefix() {
        assert_eq!(extract_bearer("abc123"), None);
    }

    #[test]
    fn extract_bearer_empty() {
        assert_eq!(extract_bearer(""), None);
    }

    #[test]
    fn extract_bearer_wrong_case() {
        assert_eq!(extract_bearer("bearer abc123"), None);
    }
}
