//! JWT authentication module for the XFChess signing service.
//!
//! This module provides JWT token issuance and verification for wallet-based authentication.
//! Tokens are used to authorize API requests for session management and game operations.

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Token time-to-live in seconds (2 hours)
const TOKEN_TTL_SECS: i64 = 7200;

/// JWT claims structure containing wallet identity and expiration.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Wallet public key (base58 encoded)
    pub sub: String,
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
        let claims = Claims {
            sub: wallet_pubkey.to_string(),
            exp: Utc::now().timestamp() + TOKEN_TTL_SECS,
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
