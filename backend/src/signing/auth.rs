use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const TOKEN_TTL_SECS: i64 = 7200; // 2 hours

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// wallet pubkey (base58)
    pub sub: String,
    /// expiry unix timestamp
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
        let claims = Claims {
            sub: wallet_pubkey.to_string(),
            exp: Utc::now().timestamp() + TOKEN_TTL_SECS,
        };
        encode(&Header::default(), &claims, &self.encoding)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(token, &self.decoding, &Validation::default())?;
        Ok(data.claims)
    }
}

/// Extract Bearer token from Authorization header value.
pub fn extract_bearer(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ")
}
