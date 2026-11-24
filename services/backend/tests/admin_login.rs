//! Admin sign-in: username and password, the session token, and the ways it must fail.
mod common;

use anyhow::Context as _;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use backend::infrastructure::admin_session::hash_password;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};

const JWT_SECRET: &str = "test-secret-not-for-production";

async fn admin_get(app: &common::TestApp, session: &str) -> anyhow::Result<StatusCode> {
    let request = Request::builder()
        .uri("/admin/audit-log?limit=1")
        .method("GET")
        .header("X-Admin-Session", session)
        .body(Body::empty())
        .context("request")?;
    Ok(app.send(request).await?.0)
}

async fn login(
    app: &common::TestApp,
    username: &str,
    password: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    app.post_json(
        "/admin/auth/login",
        &json!({ "username": username, "password": password }),
    )
    .await
}

fn set_users(value: &str) {
    std::env::set_var("ADMIN_USERS", value);
}

// One test: the accounts live in the process environment, so the steps run in order.
#[tokio::test]
async fn an_admin_signs_in_with_a_password_and_every_other_path_is_refused() -> anyhow::Result<()>
{
    let first = hash_password("correct horse battery").map_err(anyhow::Error::msg)?;
    let second = hash_password("a different password").map_err(anyhow::Error::msg)?;
    std::env::set_var("JWT_SECRET", JWT_SECRET);
    set_users(&format!("tino:{first}"));
    let app = common::spawn_app().await?;

    // Sign in.
    let (status, session) = login(&app, "tino", "correct horse battery").await?;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["actor"], "tino");
    let token = session["token"].as_str().context("token")?.to_string();
    assert_eq!(admin_get(&app, &token).await?, StatusCode::OK);

    // A wrong password and an unknown name are refused alike.
    let (status, _) = login(&app, "tino", "correct horse").await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "wrong password");
    let (status, _) = login(&app, "nobody", "correct horse battery").await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unknown name");
    let (status, _) = login(&app, "tino", "").await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "empty password");

    // Tokens that are not admin sessions are refused.
    assert_eq!(admin_get(&app, "").await?, StatusCode::UNAUTHORIZED);
    assert_eq!(admin_get(&app, "garbage").await?, StatusCode::UNAUTHORIZED);
    let player = app.state.jwt.issue("tino")?;
    assert_eq!(
        admin_get(&app, &player).await?,
        StatusCode::UNAUTHORIZED,
        "a player token is not an admin session"
    );
    let plain = encode(
        &Header::default(),
        &json!({ "sub": "tino", "kind": "admin-session", "pw": "", "exp": 4_000_000_000u64 }),
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )?;
    assert_eq!(
        admin_get(&app, &plain).await?,
        StatusCode::UNAUTHORIZED,
        "a token made with the plain JWT secret is not an admin session"
    );

    // Changing the password ends the session; the new password signs in.
    set_users(&format!("tino:{second}"));
    assert_eq!(admin_get(&app, &token).await?, StatusCode::UNAUTHORIZED);
    let (status, _) = login(&app, "tino", "correct horse battery").await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "old password");
    let (status, session) = login(&app, "tino", "a different password").await?;
    assert_eq!(status, StatusCode::OK, "{session}");
    let token = session["token"].as_str().context("token")?.to_string();
    assert_eq!(admin_get(&app, &token).await?, StatusCode::OK);

    // Removing the account ends its session.
    set_users(&format!("other:{second}"));
    assert_eq!(admin_get(&app, &token).await?, StatusCode::UNAUTHORIZED);

    // Five wrong passwords lock an account, even against the right one.
    set_users(&format!("locked:{second}"));
    for _ in 0..5 {
        let (status, _) = login(&app, "locked", "not it").await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = login(&app, "locked", "a different password").await?;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    // With no accounts configured there is nothing to sign in to.
    std::env::remove_var("ADMIN_USERS");
    let (status, _) = login(&app, "tino", "a different password").await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    Ok(())
}
