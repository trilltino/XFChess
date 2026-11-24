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
    pub static ADMIN_ACTOR: String;
}

pub fn current_admin_actor() -> String {
    ADMIN_ACTOR
        .try_with(|a| a.clone())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Admits a request that carries a valid admin session (see `admin_session`) and records the
/// signed-in admin as the actor for the audit log.
pub async fn require_admin(request: Request, next: Next) -> Result<Response, StatusCode> {
    let actor = {
        let token = request
            .headers()
            .get(super::admin_session::SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        super::admin_session::actor_for_session(token).inspect_err(|_| {
            tracing::debug!("[auth] admin request without a valid session");
        })?
    };
    Ok(ADMIN_ACTOR.scope(actor, next.run(request)).await)
}

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
        // Await audit writes so a crash cannot lose a fire-and-forget entry.
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

pub async fn require_relay_or_jwt(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(token) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if let Ok(claims) = state.jwt.verify(token) {
            if state
                .store
                .token_is_revoked(&claims.sub, claims.iat)
                .await
                .map_err(|error| {
                    tracing::error!("Revocation lookup failed: {error}");
                    StatusCode::SERVICE_UNAVAILABLE
                })?
            {
                return Err(StatusCode::UNAUTHORIZED);
            }
            request.extensions_mut().insert(AuthedWallet(claims.sub));
            return Ok(next.run(request).await);
        }
        // Invalid token → fall through to the relay-secret path (dual-accept).
    }

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
    use anyhow::Context as _;
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

    fn guarded() -> Router {
        Router::new()
            .route("/test", get(|| async { current_admin_actor() }))
            .layer(axum::middleware::from_fn(require_admin))
            .nest(
                "/admin/auth",
                crate::infrastructure::admin_session::routes(),
            )
    }

    fn admin_get(token: Option<&str>) -> anyhow::Result<Request<Body>> {
        let mut builder = Request::builder().uri("/test");
        if let Some(token) = token {
            builder = builder.header("X-Admin-Session", token);
        }
        builder.body(Body::empty()).context("test request")
    }

    async fn body_text(response: Response) -> anyhow::Result<String> {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Only a session from `/admin/auth/login` opens an admin route, and the route sees who
    /// signed in. Removing the account ends the session.
    #[tokio::test]
    async fn admin_routes_need_a_session_from_the_login() -> anyhow::Result<()> {
        let _env = crate::test_env::lock().await;
        let hash = crate::infrastructure::admin_session::hash_password("correct horse")
            .map_err(anyhow::Error::msg)?;
        std::env::set_var("JWT_SECRET", "test-secret-not-for-production");
        std::env::set_var("ADMIN_USERS", format!("tino:{hash}"));
        let app = guarded();

        let login = Request::builder()
            .uri("/admin/auth/login")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"username":"tino","password":"correct horse"}"#,
            ))
            .context("test request")?;
        let response = app.clone().oneshot(login).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let session: serde_json::Value = serde_json::from_str(&body_text(response).await?)?;
        let token = session["token"].as_str().context("token")?.to_string();

        let response = app.clone().oneshot(admin_get(Some(&token))?).await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_text(response).await?, "tino");

        for refused in [None, Some(""), Some("not-a-session")] {
            let status = app.clone().oneshot(admin_get(refused)?).await?.status();
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused:?}");
        }

        std::env::remove_var("ADMIN_USERS");
        let status = app.clone().oneshot(admin_get(Some(&token))?).await?.status();
        std::env::remove_var("JWT_SECRET");
        assert_eq!(status, StatusCode::UNAUTHORIZED, "account removed");
        Ok(())
    }
}
