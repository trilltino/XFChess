//! Admin wallet sign-in: challenge, signature, session token, and the ways it must fail.
mod common;

use anyhow::Context as _;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use solana_sdk::signature::{Keypair, Signer};

async fn admin_get(app: &common::TestApp, token: &str) -> anyhow::Result<StatusCode> {
    let request = Request::builder()
        .uri("/admin/audit-log?limit=1")
        .method("GET")
        .header("X-API-Key", token)
        .body(Body::empty())
        .context("request")?;
    Ok(app.send(request).await?.0)
}

async fn challenge(app: &common::TestApp, wallet: &Keypair) -> anyhow::Result<(StatusCode, Value)> {
    app.post_json(
        "/admin/auth/challenge",
        &json!({ "wallet": wallet.pubkey().to_string() }),
    )
    .await
}

async fn login(
    app: &common::TestApp,
    wallet: &Keypair,
    challenge: &Value,
    signer: &Keypair,
) -> anyhow::Result<(StatusCode, Value)> {
    let message = challenge["message"].as_str().context("message")?;
    let signature = signer.sign_message(message.as_bytes());
    app.post_json(
        "/admin/auth/login",
        &json!({
            "wallet": wallet.pubkey().to_string(),
            "nonce": challenge["nonce"],
            "signature": signature.to_string(),
        }),
    )
    .await
}

// One test: the allowlist lives in the process environment, so the steps run in order.
#[tokio::test]
async fn an_admin_wallet_signs_in_and_every_other_path_is_refused() -> anyhow::Result<()> {
    let admin = Keypair::new();
    let stranger = Keypair::new();
    std::env::set_var("JWT_SECRET", "test-secret-not-for-production");
    std::env::set_var("ADMIN_WALLETS", format!("tino:{}", admin.pubkey()));
    let app = common::spawn_app().await?;

    // Sign in.
    let (status, issued) = challenge(&app, &admin).await?;
    assert_eq!(status, StatusCode::OK, "{issued}");
    assert!(issued["message"]
        .as_str()
        .context("message")?
        .contains(&admin.pubkey().to_string()));
    let (status, session) = login(&app, &admin, &issued, &admin).await?;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["actor"], "wallet:tino");
    let token = session["token"].as_str().context("token")?.to_string();
    assert!(token.starts_with("xfa."));
    assert_eq!(admin_get(&app, &token).await?, StatusCode::OK);

    // A challenge is single use.
    let (status, _) = login(&app, &admin, &issued, &admin).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "replay");

    // A wallet that is not on the list gets no challenge.
    let (status, _) = challenge(&app, &stranger).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A signature from another key is refused, and it still spends the challenge.
    let (_, issued) = challenge(&app, &admin).await?;
    let (status, _) = login(&app, &admin, &issued, &stranger).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "wrong signer");
    let (status, _) = login(&app, &admin, &issued, &admin).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "spent by the failed attempt");

    // A challenge issued to one wallet cannot be completed for another.
    let (_, issued) = challenge(&app, &admin).await?;
    let (status, _) = login(&app, &stranger, &issued, &stranger).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another wallet");

    // Tokens that are not admin sessions are refused.
    assert_eq!(admin_get(&app, "xfa.garbage").await?, StatusCode::UNAUTHORIZED);
    let plain = encode(
        &Header::default(),
        &json!({ "sub": admin.pubkey().to_string(), "exp": 4_000_000_000u64 }),
        &EncodingKey::from_secret(b"test-secret-not-for-production"),
    )?;
    assert_eq!(
        admin_get(&app, &format!("xfa.{plain}")).await?,
        StatusCode::UNAUTHORIZED,
        "a token made with the plain secret is not an admin session"
    );

    // Removing the wallet from the list ends its session.
    std::env::set_var("ADMIN_WALLETS", format!("other:{}", stranger.pubkey()));
    assert_eq!(admin_get(&app, &token).await?, StatusCode::UNAUTHORIZED);

    // With no wallets configured there is nothing to sign in to.
    std::env::remove_var("ADMIN_WALLETS");
    let (status, _) = challenge(&app, &admin).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    Ok(())
}
