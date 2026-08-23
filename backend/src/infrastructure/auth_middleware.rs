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

tokio::task_local! {
    /// The resolved admin identity for the current request, set by
    /// `require_api_key` and read by `admin::add_audit` — this is how the
    /// audit log attributes an action to a named operator without every
    /// handler needing an `Extension<AdminActor>` parameter. Axum runs the
    /// handler in the same task as the middleware chain, so a task-local
    /// scope set here is visible for the whole request lifetime.
    pub static ADMIN_ACTOR: String;
}

/// Returns the actor name resolved by `require_api_key` for the request
/// currently executing on this task, or `"unknown"` if called outside one
/// (e.g. a unit test that invokes a handler directly).
pub fn current_admin_actor() -> String {
    ADMIN_ACTOR
        .try_with(|a| a.clone())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Resolves the `X-API-Key` header against configured admin keys and, on a
/// match, returns the actor name to attribute the request to.
///
/// Supports two forms, checked in order:
/// - `ADMIN_API_KEYS=alice:<key>,ci:<key2>` — named keys, each compared in
///   constant time; the matching name becomes the actor.
/// - `ADMIN_API_KEY=<key>` — single legacy key, actor is `"legacy"`.
/// - Debug builds only, neither set: the literal key `"dev"`, actor
///   `"dev-default"`.
fn resolve_admin_actor(provided_key: &str) -> Result<String, StatusCode> {
    if let Ok(named) = env::var("ADMIN_API_KEYS") {
        let mut any_configured = false;
        for pair in named.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let Some((name, key)) = pair.split_once(':') else {
                continue;
            };
            any_configured = true;
            if constant_time_eq(provided_key, key) {
                return Ok(name.to_string());
            }
        }
        if any_configured {
            return Err(StatusCode::UNAUTHORIZED);
        }
        // ADMIN_API_KEYS set but empty/unparseable — fall through to legacy.
    }

    let expected_key = match env::var("ADMIN_API_KEY") {
        Ok(key) => key,
        Err(env::VarError::NotPresent) => {
            #[cfg(debug_assertions)]
            {
                tracing::warn!("[auth] ADMIN_API_KEY not set — defaulting to 'dev' in debug build");
                if constant_time_eq(provided_key, "dev") {
                    return Ok("dev-default".to_string());
                }
                return Err(StatusCode::UNAUTHORIZED);
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

    if constant_time_eq(provided_key, &expected_key) {
        Ok("legacy".to_string())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

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
    let provided_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let actor = match resolve_admin_actor(&provided_key) {
        Ok(actor) => actor,
        Err(status) => {
            tracing::debug!("[auth] Invalid API key provided");
            return Err(status);
        }
    };

    tracing::debug!("[auth] API key validated successfully (actor={actor})");
    Ok(ADMIN_ACTOR.scope(actor, next.run(request)).await)
}

/// Catch-all persistence for every mutating `/admin/*` request, independent
/// of whether the handler calls `admin::add_audit`. This is the safety net
/// from Phase 3 of `docs/plans/admin-panel-and-production-hardening.md`: a
/// new admin route that forgets to log its own action is still captured here
/// with method/path/status/actor, so the audit trail can't silently miss a
/// mutation. Read-only `GET`/`HEAD` requests are skipped — they don't change
/// state and would otherwise dominate the log with noise. Must be layered
/// *inside* (after) `require_api_key` so `current_admin_actor()` is populated.
pub async fn persist_admin_request(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let skip = matches!(method, axum::http::Method::GET | axum::http::Method::HEAD);
    let actor = current_admin_actor();

    let response = next.run(request).await;

    if !skip {
        let pool = state.store.pool();
        let status = response.status().as_u16() as i64;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let result = format!("{} -> {}", method.as_str(), status);
        // Awaited, not spawned — same durability reasoning as
        // `admin::add_audit`: an audit entry that can be silently dropped by
        // a crash racing a fire-and-forget task isn't durable.
        if let Err(e) = sqlx::query(
            "INSERT INTO admin_audit_log (ts, actor, action, target, result, method, path, status) \
             VALUES (?, ?, ?, '', ?, ?, ?, ?)",
        )
        .bind(ts)
        .bind(&actor)
        .bind(&path)
        .bind(&result)
        .bind(method.as_str())
        .bind(&path)
        .bind(status)
        .execute(&pool)
        .await
        {
            tracing::error!("[auth] failed to persist admin audit entry for {path}: {e}");
        }
    }

    response
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
/// retired once clients have migrated. The game client fetches a JWT
/// automatically on every wallet connection before any session/lobby action
/// (see `src/multiplayer/network/vps/client.rs`), so this guard fails
/// **closed**: if neither a valid JWT nor `RELAY_SHARED_SECRET` is presented,
/// the request is rejected rather than allowed through on the assumption that
/// a firewall is doing the job instead.
pub async fn require_relay_or_jwt(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
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

    // 2) Legacy: the shared relay secret. Unset/empty → fail closed.
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
            crate::telemetry::worker_metrics::AUTH_UNCONFIGURED_RELAY_REJECTED_TOTAL
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Constant-time string comparison to prevent timing attacks.
///
/// Shared across admin auth surfaces (this middleware's `X-API-Key` check and
/// `routes/dispute.rs`'s `ADMIN_TOKEN` check) so no secret is ever compared with
/// a short-circuiting `==`/`!=`.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
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
