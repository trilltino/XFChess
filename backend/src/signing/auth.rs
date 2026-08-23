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

/// Request extension inserted by the dual-accept auth middleware when — and
/// only when — a request authenticated via a per-user JWT. Its presence means
/// the caller cryptographically proved control of this wallet; the legacy
/// relay-secret path deliberately never inserts it, because a shared secret
/// held by every game client proves nothing about *which* player is calling.
///
/// Handlers must not read this as `Option<Extension<AuthedWallet>>`. That shape
/// is what made the relay secret a universal impersonation key: every
/// per-caller check was written `if let Some(authed) = authed { ... }`, so a
/// request arriving without a proven identity **skipped** the check instead of
/// failing it. Use the [`RequireWallet`] extractor instead — it fails closed.
#[derive(Clone, Debug)]
pub struct AuthedWallet(pub String);

/// Fail-closed extractor for "this route acts on behalf of exactly one wallet".
///
/// Yields the caller's cryptographically proven wallet, or rejects the request
/// with `401` when no [`AuthedWallet`] was established. Because it is a
/// mandatory extractor rather than an `Option`, a handler simply cannot be
/// written in a way that silently proceeds without an identity — the "forgot to
/// check" failure mode becomes impossible rather than merely discouraged.
pub struct RequireWallet(pub String);

impl<S> axum::extract::FromRequestParts<S> for RequireWallet
where
    S: Send + Sync,
{
    type Rejection = (axum::http::StatusCode, String);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<AuthedWallet>() {
            Some(AuthedWallet(wallet)) => Ok(RequireWallet(wallet.clone())),
            None => Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "This endpoint acts on behalf of a specific wallet and requires a per-user \
                 bearer token. Authenticate via /api/auth/siws-challenge + /api/auth/siws-verify; \
                 the shared relay secret is not accepted here."
                    .to_string(),
            )),
        }
    }
}

impl RequireWallet {
    /// Asserts the authenticated caller is exactly `claimed`, for handlers that
    /// take the acted-on wallet in their request body. Mirrors
    /// `routes::auth::require_caller_owns_wallet` for the extension-based path.
    pub fn require_is(&self, claimed: &str) -> Result<(), (axum::http::StatusCode, String)> {
        if self.0 == claimed {
            return Ok(());
        }
        Err((
            axum::http::StatusCode::FORBIDDEN,
            format!(
                "Authenticated wallet {} does not match the wallet this request claims to act on \
                 ('{claimed}')",
                self.0
            ),
        ))
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
