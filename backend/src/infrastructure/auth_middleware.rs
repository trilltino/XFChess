//! API Key authentication middleware for admin routes.
//!
//! Protects sensitive endpoints by requiring a valid `X-API-Key` header.
//! The expected key is set via the `ADMIN_API_KEY` environment variable.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::env;

/// Middleware function that validates the X-API-Key header.
///
/// # Arguments
/// * `request` - The incoming request
/// * `next` - The next middleware/handler in the chain
///
/// # Returns
/// Response with 401 Unauthorized if API key is missing or invalid,
/// otherwise passes through to the next handler.
pub async fn require_api_key(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get expected API key from environment
    let expected_key = env::var("ADMIN_API_KEY").unwrap_or_default();
    
    // If no API key is configured, reject all requests (fail secure)
    if expected_key.is_empty() {
        tracing::warn!("[auth] ADMIN_API_KEY not configured, rejecting request");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    
    // Extract X-API-Key header
    let provided_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    
    // Validate key using constant-time comparison
    if !constant_time_eq(&provided_key, &expected_key) {
        tracing::warn!("[auth] Invalid API key provided");
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    tracing::debug!("[auth] API key validated successfully");
    Ok(next.run(request).await)
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("secret123", "secret123"));
        assert!(!constant_time_eq("secret123", "secret124"));
        assert!(!constant_time_eq("secret", "secret123"));
        assert!(!constant_time_eq("", "secret"));
    }
}
