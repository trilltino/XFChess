#![allow(dead_code)]
use anyhow::Context as _;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use backend::db::repository::GameRepository;
use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::{AppState, SigningConfig};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

pub fn unique_db_url(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sqlite:file:xfchess_e2e_{tag}_{n}_{nanos}?mode=memory&cache=shared")
}

pub fn test_config() -> SigningConfig {
    SigningConfig {
        port: 0,
        solana_rpc_url: "http://127.0.0.1:9".into(),
        solana_mainnet_rpc_url: None,
        er_rpc_url: "http://127.0.0.1:9".into(),
        magic_router_rpc_url: "http://127.0.0.1:9".into(),
        program_id: "6idpQeH5YzXUT4sg8mG8dzxdK7ytmxfEyWkzUcNJJv4N".into(),
        jwt_secret: "test-secret-not-for-production".into(),
        identity_encryption_key: "0".repeat(64),
        identity_salt: "0".repeat(64),
        fee_payer_keys: vec![],
        vps_authority_key: None,
        kyc_authority_key: None,
        link_authority_key: None,
        treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
        admin_token: Some("test-admin-token".into()),
        tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
        usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
        lichess_client_id: String::new(),
        allowed_origins: vec![],
    }
}

pub const ADMIN_NAME: &str = "test-admin";
pub const ADMIN_PASSWORD: &str = "test admin password";

/// Sets up the test admin account in the process environment, once per test binary. Admin
/// sessions are signed with a key derived from `JWT_SECRET` and accounts come from `ADMIN_USERS`.
pub fn configure_admin_account() -> anyhow::Result<()> {
    static CONFIGURED: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    CONFIGURED
        .get_or_init(|| {
            let hash = backend::infrastructure::admin_session::hash_password(ADMIN_PASSWORD)?;
            std::env::set_var("JWT_SECRET", test_config().jwt_secret);
            std::env::set_var("ADMIN_USERS", format!("{ADMIN_NAME}:{hash}"));
            Ok(())
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

/// A session for the test admin, signed in through `router`'s `/admin/auth/login` once per test
/// binary. It goes in the `X-Admin-Session` header.
pub async fn admin_session(router: Router) -> anyhow::Result<String> {
    static TOKEN: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    configure_admin_account()?;
    TOKEN
        .get_or_try_init(|| async move {
            let body = serde_json::json!({ "username": ADMIN_NAME, "password": ADMIN_PASSWORD });
            let request = Request::builder()
                .uri("/admin/auth/login")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))
                .context("test setup failed")?;
            let response = router.oneshot(request).await.context("test setup failed")?;
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .context("test setup failed")?;
            anyhow::ensure!(
                status == StatusCode::OK,
                "test admin sign-in failed: {status} {}",
                String::from_utf8_lossy(&bytes)
            );
            let reply: Value = serde_json::from_slice(&bytes)?;
            Ok(reply["token"]
                .as_str()
                .context("the sign-in reply has no token")?
                .to_string())
        })
        .await
        .cloned()
}

pub struct TestApp {
    pub state: AppState,
}

impl TestApp {
    pub fn router(&self) -> Router {
        build_app_router(self.state.clone()).with_state(self.state.clone())
    }

    /// A session for the test admin (see [`admin_session`]).
    pub async fn admin_token(&self) -> anyhow::Result<String> {
        admin_session(self.router()).await
    }

    pub async fn get(&self, uri: &str) -> anyhow::Result<(StatusCode, Value)> {
        Ok({
            let req = Request::builder()
                .uri(uri)
                .method("GET")
                .body(Body::empty())
                .context("test setup failed")?;
            self.send(req).await?
        })
    }

    pub async fn post_json(&self, uri: &str, body: &Value) -> anyhow::Result<(StatusCode, Value)> {
        Ok({
            let req = Request::builder()
                .uri(uri)
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(body).context("test setup failed")?,
                ))
                .context("test setup failed")?;
            self.send(req).await?
        })
    }

    pub async fn admin_request(
        &self,
        method: &str,
        uri: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<(StatusCode, Value)> {
        Ok({
            let mut builder = Request::builder()
                .uri(uri)
                .method(method)
                .header("X-Admin-Session", self.admin_token().await?);
            let body = match body {
                Some(b) => {
                    builder = builder.header("content-type", "application/json");
                    Body::from(serde_json::to_vec(b).context("test setup failed")?)
                }
                None => Body::empty(),
            };
            let req = builder.body(body).context("test setup failed")?;
            self.send(req).await?
        })
    }

    pub async fn send(&self, req: Request<Body>) -> anyhow::Result<(StatusCode, Value)> {
        Ok({
            let resp = self
                .router()
                .oneshot(req)
                .await
                .context("test setup failed")?;
            let status = resp.status();
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .context("test setup failed")?;
            let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            (status, value)
        })
    }

    pub async fn get_text(&self, uri: &str) -> anyhow::Result<(StatusCode, String)> {
        Ok({
            let req = Request::builder()
                .uri(uri)
                .method("GET")
                .body(Body::empty())
                .context("test setup failed")?;
            let resp = self
                .router()
                .oneshot(req)
                .await
                .context("test setup failed")?;
            let status = resp.status();
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .context("test setup failed")?;
            (status, String::from_utf8_lossy(&bytes).into_owned())
        })
    }

    pub fn repo(&self) -> GameRepository {
        GameRepository::new(self.state.store.pool())
    }
}

pub async fn spawn_app() -> anyhow::Result<TestApp> {
    spawn_app_on(&unique_db_url("session"), &unique_db_url("vault")).await
}

/// Build a fresh AppState over existing SQLite databases to simulate restart.
/// Shared in-memory databases survive while an earlier pool remains open.
pub async fn spawn_app_on(session_url: &str, vault_url: &str) -> anyhow::Result<TestApp> {
    Ok({
        let pools = initialize_pools(session_url, vault_url)
            .await
            .context("init pools")?;
        run_migrations(&pools).await.context("run migrations")?;

        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await?;

        let state = AppState::new_with_background_tasks(
            test_config(),
            pools.session_pool.clone(),
            pools.vault_pool.clone(),
            Arc::new(tournament_store),
            false,
        )?;
        // Same startup step as `server::run`: restore persisted relay rooms.
        state
            .p2p_relay_store
            .hydrate(&state.p2p_relay)
            .await
            .context("hydrate relay rooms")?;

        TestApp { state }
    })
}

/// An `X-Node-Auth` token proving `key`'s node id, issued `age_secs` ago.
pub fn node_token_aged(key: &solana_sdk::signature::Keypair, age_secs: i64) -> String {
    use solana_sdk::signature::Signer as _;
    let node_id = key.pubkey().to_string();
    let issued_at = chrono::Utc::now().timestamp() - age_secs;
    let signature =
        key.sign_message(backend::signing::node_auth::signable(&node_id, issued_at).as_bytes());
    format!("v1.{node_id}.{issued_at}.{signature}")
}

pub fn node_token(key: &solana_sdk::signature::Keypair) -> String {
    node_token_aged(key, 0)
}

impl TestApp {
    /// A request made as the network node that owns `key`.
    pub async fn as_node(
        &self,
        key: &solana_sdk::signature::Keypair,
        method: &str,
        uri: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut builder = Request::builder()
            .uri(uri)
            .method(method)
            .header("x-node-auth", node_token(key));
        let body = match body {
            Some(b) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(b).context("test setup failed")?)
            }
            None => Body::empty(),
        };
        self.send(builder.body(body).context("test setup failed")?)
            .await
    }
}
