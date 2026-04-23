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
    // Get expected API key from environment with explicit error handling
    let expected_key = match env::var("ADMIN_API_KEY") {
        Ok(key) => key,
        Err(env::VarError::NotPresent) => {
            tracing::warn!("[auth] ADMIN_API_KEY not configured, rejecting request");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(env::VarError::NotUnicode(_)) => {
            tracing::error!("[auth] ADMIN_API_KEY contains invalid UTF-8, rejecting request");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    
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
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    #[test]
    fn test_constant_time_eq() {
        // Basic equality
        assert!(constant_time_eq("secret123", "secret123"));
        assert!(constant_time_eq("", ""));
        assert!(constant_time_eq("a", "a"));
        
        // Inequality
        assert!(!constant_time_eq("secret123", "secret124"));
        assert!(!constant_time_eq("secret", "secret123"));
        assert!(!constant_time_eq("", "secret"));
        assert!(!constant_time_eq("secret", ""));
        
        // Different lengths
        assert!(!constant_time_eq("short", "longer"));
        assert!(!constant_time_eq("a", "ab"));
        
        // Same length, different content
        assert!(!constant_time_eq("abc", "def"));
        assert!(!constant_time_eq("123", "456"));
    }

    #[tokio::test]
    #[ignore = "Flaky: race condition with parallel tests modifying global env vars"]
    async fn test_require_api_key_missing_env_var() {
        // Remove env var if it exists
        std::env::remove_var("ADMIN_API_KEY");

        let app = Router::new()
            .route("/test", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn(require_api_key));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_require_api_key_valid() {
        std::env::set_var("ADMIN_API_KEY", "test-secret-key");

        let app = Router::new()
            .route("/test", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn(require_api_key));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("X-API-Key", "test-secret-key")
                    .body(Body::empty())
                    .expect("Failed to build test request"),
            )
            .await
            .expect("Failed to execute test request");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_api_key_invalid() {
        std::env::set_var("ADMIN_API_KEY", "test-secret-key");

        let app = Router::new()
            .route("/test", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn(require_api_key));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("X-API-Key", "wrong-key")
                    .body(Body::empty())
                    .expect("Failed to build test request"),
            )
            .await
            .expect("Failed to execute test request");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_require_api_key_missing_header() {
        std::env::set_var("ADMIN_API_KEY", "test-secret-key");

        let app = Router::new()
            .route("/test", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn(require_api_key));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_require_api_key_empty_header() {
        std::env::set_var("ADMIN_API_KEY", "test-secret-key");

        let app = Router::new()
            .route("/test", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn(require_api_key));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("X-API-Key", "")
                    .body(Body::empty())
                    .expect("Failed to build test request"),
            )
            .await
            .expect("Failed to execute test request");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
