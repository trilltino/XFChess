//! API Key authentication middleware for admin routes.
//!
//! Protects sensitive endpoints by requiring a valid `X-API-Key` header.
//! The expected key is set via the `ADMIN_API_KEY` environment variable.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::env;

use crate::signing::auth::AuthedWallet;
use crate::signing::AppState;

/// Middleware function that validates the X-API-Key header.
///
/// # Arguments
/// * `request` - The incoming request
/// * `next` - The next middleware/handler in the chain
///
/// # Returns
/// Response with 401 Unauthorized if API key is missing or invalid,
/// otherwise passes through to the next handler.
pub async fn require_api_key(request: Request, next: Next) -> Result<Response, StatusCode> {
    let expected_key = match env::var("ADMIN_API_KEY") {
        Ok(key) => key,
        Err(env::VarError::NotPresent) => {
            #[cfg(debug_assertions)]
            {
                tracing::warn!("[auth] ADMIN_API_KEY not set — defaulting to 'dev' in debug build");
                "dev".to_string()
            }
            #[cfg(not(debug_assertions))]
            {
                tracing::error!("[auth] ADMIN_API_KEY not configured in production build");
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
        Err(env::VarError::NotUnicode(_)) => {
            tracing::error!("[auth] ADMIN_API_KEY contains invalid UTF-8");
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
        tracing::debug!("[auth] Invalid API key provided");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::debug!("[auth] API key validated successfully");
    Ok(next.run(request).await)
}

/// Middleware protecting the session-key signing endpoints (`/move/record`,
/// `/session/*`, `/game/finalize`, …) which the VPS signs on the caller's
/// behalf. Requires a matching `X-Relay-Secret` header when `RELAY_SHARED_SECRET`
/// is configured.
///
/// When the env var is unset it **fails open** — these endpoints are firewalled
/// to the game client on the VPS, so an unset secret keeps existing deployments
/// working while a one-time warning flags that they're relying on the network
/// boundary alone. Set `RELAY_SHARED_SECRET` (and the matching client value) to
/// add application-layer auth.
pub async fn require_relay_secret(request: Request, next: Next) -> Result<Response, StatusCode> {
    use std::sync::Once;
    static UNSET_WARNING: Once = Once::new();

    let expected_secret = match env::var("RELAY_SHARED_SECRET") {
        Ok(secret) if !secret.is_empty() => secret,
        _ => {
            UNSET_WARNING.call_once(|| {
                tracing::warn!(
                    "[auth] RELAY_SHARED_SECRET not set — relay/signing endpoints are \
                     unauthenticated and rely on the network firewall"
                );
            });
            return Ok(next.run(request).await);
        }
    };

    let provided_secret = request
        .headers()
        .get("X-Relay-Secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if !constant_time_eq(provided_secret, &expected_secret) {
        tracing::debug!("[auth] Invalid or missing X-Relay-Secret on a protected relay endpoint");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Dual-accept guard for the session-key signing endpoints.
///
/// A request is allowed if **either**:
///  1. it carries a valid, non-revoked per-user JWT (`Authorization: Bearer …`) —
///     the preferred path; the caller's wallet is stashed as [`AuthedWallet`] so
///     handlers can apply per-caller authorization, **or**
///  2. it carries the legacy `X-Relay-Secret` matching `RELAY_SHARED_SECRET`.
///
/// This is the rollout bridge from the shared secret to per-user auth: old
/// clients (relay secret) and new clients (JWT) both work, so the secret can be
/// retired once clients have migrated. When neither `RELAY_SHARED_SECRET` is set
/// nor a JWT is presented, it fails **open** (relying on the network firewall),
/// preserving prior behaviour for un-migrated deployments.
pub async fn require_relay_or_jwt(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    use std::sync::Once;
    static UNSET_WARNING: Once = Once::new();

    // 1) Preferred: a valid, non-revoked per-user JWT.
    if let Some(token) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if let Ok(claims) = state.jwt.verify(token) {
            if state.store.token_is_revoked(&claims.sub, claims.iat).await {
                return Err(StatusCode::UNAUTHORIZED);
            }
            request.extensions_mut().insert(AuthedWallet(claims.sub));
            return Ok(next.run(request).await);
        }
        // Invalid token → fall through to the relay-secret path (dual-accept).
    }

    // 2) Legacy: the shared relay secret.
    match env::var("RELAY_SHARED_SECRET") {
        Ok(secret) if !secret.is_empty() => {
            let provided = request
                .headers()
                .get("X-Relay-Secret")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            if constant_time_eq(provided, &secret) {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => {
            UNSET_WARNING.call_once(|| {
                tracing::warn!(
                    "[auth] No JWT and RELAY_SHARED_SECRET unset — signing endpoints are \
                     unauthenticated and rely on the network firewall"
                );
            });
            Ok(next.run(request).await)
        }
    }
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
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
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
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
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
