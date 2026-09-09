use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const DEFAULT_TOKEN_TTL_SECS: i64 = 604_800;

fn token_ttl_secs() -> i64 {
    std::env::var("JWT_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_TOKEN_TTL_SECS)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub iat: i64,
    pub exp: i64,
}

pub struct JwtIssuer {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtIssuer {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn issue(&self, wallet_pubkey: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: wallet_pubkey.to_string(),
            iat: now,
            exp: now + token_ttl_secs(),
        };
        encode(&Header::default(), &claims, &self.encoding)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(token, &self.decoding, &Validation::default())?;
        Ok(data.claims)
    }
}

#[derive(Clone, Debug)]
pub struct AuthedWallet(pub String);

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
