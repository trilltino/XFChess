//! In-process HTTP end-to-end tests (Tier T1; see docs/plans/e2e-testing.md).
//!
//! `spawn_app()` reproduces the real server startup — `initialize_pools` →
//! `run_migrations` → `SessionStore::init` (which also applies the 013–016
//! schema) → `AppState::new` → `build_app_router` — against a private
//! shared-cache in-memory SQLite, then drives the *real* router with
//! `tower::ServiceExt::oneshot`. No network, no validator, no mocks of our own
//! code. Flows are restricted to the chain-free seams (the Solana RPC endpoint
//! is configured but never hit by these routes).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

use backend::db::repository::GameRepository;
use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
use backend::signing::identity::IdentityVault;
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::signing::{AppState, SigningConfig};
use std::sync::Mutex;

/// Per-test unique shared-cache in-memory DB name so the 16-connection pool all
/// sees the same database and tests don't collide.
fn unique_db_url(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sqlite:file:xfchess_e2e_{tag}_{n}_{nanos}?mode=memory&cache=shared")
}

/// Test config with valid pubkeys / 32-byte hex keys; RPC URLs point nowhere
/// real because no tested route performs a Solana RPC call.
fn test_config() -> SigningConfig {
    SigningConfig {
        port: 0,
        solana_rpc_url: "http://127.0.0.1:9".into(),
        solana_mainnet_rpc_url: None,
        er_rpc_url: "http://127.0.0.1:9".into(),
        magic_router_rpc_url: "http://127.0.0.1:9".into(),
        program_id: "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".into(),
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

struct TestApp {
    state: AppState,
}

impl TestApp {
    /// A fresh, serveable router (oneshot consumes it, so build per request).
    fn router(&self) -> Router {
        build_app_router(self.state.clone()).with_state(self.state.clone())
    }

    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("GET")
            .body(Body::empty())
            .unwrap();
        self.send(req).await
    }

    async fn post_json(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap();
        self.send(req).await
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, Value) {
        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    /// Raw-text variant for non-JSON endpoints (e.g. /metrics).
    async fn get_text(&self, uri: &str) -> (StatusCode, String) {
        let req = Request::builder()
            .uri(uri)
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    fn repo(&self) -> GameRepository {
        GameRepository::new(self.state.store.pool())
    }
}

async fn spawn_app() -> TestApp {
    let pools = initialize_pools(&unique_db_url("session"), &unique_db_url("vault"))
        .await
        .expect("init pools");
    run_migrations(&pools).await.expect("run migrations");

    // Only used below to create/migrate tables via `.init()` — the vault
    // itself doesn't matter since AppState::new builds the real store this
    // test actually reads/writes through.
    let schema_vault =
        IdentityVault::new(&"0".repeat(64), &"0".repeat(64)).expect("test vault");
    let session_store = SessionStore::new(pools.session_pool.clone(), schema_vault);
    session_store.init().await.expect("session store init");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

    let state = AppState::new(
        test_config(),
        pools.session_pool.clone(),
        pools.vault_pool.clone(),
        Arc::new(tournament_store),
    );
    // Social tables (some routes touch them; harmless for the rest).
    let _ = state.friends.init().await;

    TestApp { state }
}

// ── /metrics ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn metrics_endpoint_exposes_worker_counters() {
    let app = spawn_app().await;
    let (status, body) = app.get_text("/metrics").await;
    assert_eq!(status, StatusCode::OK);
    // Core + worker/anti-cheat/linkage counters must all be present.
    assert!(
        body.contains("xfchess_settlement_ticks_total"),
        "missing settlement metric:\n{body}"
    );
    assert!(
        body.contains("xfchess_anticheat_queue_depth"),
        "missing anticheat metric"
    );
    assert!(
        body.contains("xfchess_linkage_flagged_total"),
        "missing linkage metric"
    );
    assert!(
        body.contains("xfchess_prize_distribution_held_total"),
        "missing prize metric"
    );
    assert!(
        body.contains("xfchess_settlement_stale_delegated_gauge"),
        "missing stale-delegation gauge (persistency plan Phase 5 monitoring)"
    );
    assert!(
        body.contains("xfchess_auth_unconfigured_relay_rejected_total"),
        "missing unconfigured-relay auth rejection counter"
    );
}

// multi_thread flavor: detailed_health_check's Solana RPC check runs the
// blocking RpcClient inside spawn_blocking (block_in_place), same reason as
// offchain_username_does_not_imply_onchain_profile above.
#[tokio::test(flavor = "multi_thread")]
async fn health_detailed_reports_real_memory_and_disk_state() {
    let app = spawn_app().await;
    let (status, body) = app.get("/health/detailed").await;
    // "degraded" (e.g. the RPC check failing against this test's unreachable
    // RPC URL) still returns 200 — only "critical" returns 503.
    assert_eq!(status, StatusCode::OK, "{body}");

    let checks = body["checks"].as_array().expect("checks array");
    let find = |name: &str| {
        checks
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("no '{name}' check in {checks:?}"))
    };

    let memory = find("memory");
    assert_eq!(memory["status"], "ok", "memory check: {memory}");
    let memory_msg = memory["message"].as_str().unwrap_or_default();
    assert!(
        memory_msg.contains('%'),
        "memory check should report a real usage percentage, not the old \
         hardcoded placeholder string: {memory_msg}"
    );

    let disk = find("disk_space");
    let disk_msg = disk["message"].as_str().unwrap_or_default();
    assert!(
        disk_msg.contains('%'),
        "disk check should report a real usage percentage on every OS \
         (including Windows), not the old always-warning placeholder: {disk_msg}"
    );
}

// ── Blur telemetry parity (anti-cheat input boundary) ─────────────────────────

#[tokio::test]
async fn blur_telemetry_unknown_game_is_404() {
    let app = spawn_app().await;
    let (status, _) = app
        .post_json(
            "/telemetry/blur",
            &json!({ "game_id": 999001, "move_number": 1, "color": "white", "blurred": true }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn blur_telemetry_enforces_ply_parity() {
    let app = spawn_app().await;
    let game_id: u64 = 424242;
    // A session must exist for the game before telemetry is accepted.
    app.state
        .store
        .create(game_id, solana_sdk::pubkey::Pubkey::new_unique())
        .await
        .expect("create session");

    // Ply 1 is white's — correct color accepted.
    let (ok_status, _) = app
        .post_json(
            "/telemetry/blur",
            &json!({ "game_id": game_id, "move_number": 1, "color": "white", "blurred": false, "think_ms": 3000 }),
        )
        .await;
    assert_eq!(ok_status, StatusCode::NO_CONTENT);

    // Ply 1 claimed as black — parity violation rejected.
    let (bad_status, _) = app
        .post_json(
            "/telemetry/blur",
            &json!({ "game_id": game_id, "move_number": 1, "color": "black", "blurred": true }),
        )
        .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);

    // Ply 0 is invalid.
    let (zero_status, _) = app
        .post_json(
            "/telemetry/blur",
            &json!({ "game_id": game_id, "move_number": 0, "color": "white", "blurred": false }),
        )
        .await;
    assert_eq!(zero_status, StatusCode::BAD_REQUEST);
}

// ── Broadcast-delay gating (esports integrity) ────────────────────────────────

#[tokio::test]
async fn broadcast_delay_gates_public_move_feed() {
    let app = spawn_app().await;
    let repo = app.repo();
    let game = "770077";

    // Live game (delay 0): create the row, add two (now-stamped) moves.
    repo.set_broadcast_delay(game, 0).await.unwrap();
    repo.add_move_simple(game, 1, "e2e4", None, Some("fen1"), "white")
        .await
        .unwrap();
    repo.add_move_simple(game, 2, "e7e5", None, Some("fen2"), "black")
        .await
        .unwrap();

    let (status, body) = app.get(&format!("/games/moves/{game}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["moves"].as_array().unwrap().len(),
        2,
        "live feed shows all moves"
    );

    // Apply a 1-hour delay: the just-recorded moves are inside the window and
    // must disappear from the public feed.
    repo.set_broadcast_delay(game, 3600).await.unwrap();
    let (status, body) = app.get(&format!("/games/moves/{game}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["moves"].as_array().unwrap().len(),
        0,
        "delayed feed withholds recent moves"
    );

    // The delay is reported for the spectator client's pre-subscribe check.
    let (status, body) = app.get(&format!("/games/{game}/broadcast-delay")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["delay_secs"].as_i64().unwrap(), 3600);
}

// ── Game history ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn completed_game_surfaces_in_history() {
    let app = spawn_app().await;
    let repo = app.repo();
    let white = "WALLET_WHITE_E2E";
    let black = "WALLET_BLACK_E2E";

    repo.complete_game(
        "histgame1",
        Some(white),
        Some(black),
        Some("alice"),
        Some("bob"),
        Some(white),
        None,
        "test-sig",
        0.0,
    )
    .await
    .unwrap();

    let (status, body) = app.get(&format!("/games/history/{white}")).await;
    assert_eq!(status, StatusCode::OK);
    let games = body["games"].as_array().expect("games array");
    assert!(
        games.iter().any(|g| g["id"] == "histgame1"),
        "completed game should appear in player history: {body}"
    );
}

// ── Disputes (chain-free notify + status) ─────────────────────────────────────

#[tokio::test]
async fn dispute_notify_then_status() {
    let app = spawn_app().await;
    let game_id = 5150;

    let (status, body) = app
        .post_json(
            "/dispute/notify",
            &json!({
                "game_id": game_id,
                "challenger_wallet": "WALLET_CHALLENGER",
                "reason": "suspected engine use",
                "tx_signature": "sig-abc"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["case_id"], json!(format!("DISP-{game_id}")));

    // The dispute is now queryable.
    let (status, body) = app.get(&format!("/dispute/{game_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["game_id"].as_i64().unwrap(), game_id);

    // Unknown dispute → 404.
    let (status, _) = app.get("/dispute/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Auth-chain hardening (this pass) ─────────────────────────────────────────

use solana_sdk::signature::{Keypair, Signer};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl TestApp {
    /// Send a request with an optional Bearer token and optional JSON body.
    async fn send_auth(
        &self,
        method: &str,
        uri: &str,
        bearer: Option<&str>,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let mut b = Request::builder().uri(uri).method(method);
        if let Some(t) = bearer {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(body) => b
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(body).unwrap()))
                .unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        self.send(req).await
    }
}

/// The unconditional token-minting endpoint was removed; the route must be gone.
#[tokio::test]
async fn auth_issue_endpoint_is_removed() {
    let app = spawn_app().await;
    let (status, _) = app
        .post_json("/auth/issue", &json!({ "wallet_pubkey": "anything" }))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Full SIWS login produces a working JWT, and logout revokes it server-side.
#[tokio::test]
async fn siws_login_then_logout_revokes_token() {
    let app = spawn_app().await;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();

    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().expect("nonce").to_string();

    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().expect("token").to_string();

    // A JWT-protected, chain-free route works with the fresh token.
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "e2eplayer" })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "authed route should accept fresh token"
    );

    // Cross a one-second boundary so the logout cut-off is strictly after `iat`.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let (status, _) = app
        .send_auth("POST", "/api/auth/logout", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "logout should succeed");

    // The same token is now rejected.
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "e2eplayer2" })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revoked token must be rejected"
    );
}

/// A correctly-signed login with a stale timestamp is rejected (replay window).
#[tokio::test]
async fn login_rejects_stale_timestamp() {
    let app = spawn_app().await;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let ts = now_secs() - 4000; // well outside the 300s freshness window
    let sig = kp
        .sign_message(format!("xfchess:login:{ts}").as_bytes())
        .to_string();

    let (status, body) = app
        .post_json(
            "/api/auth/login",
            &json!({ "wallet": wallet, "signature": sig, "timestamp": ts }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "stale signature must be rejected: {body}"
    );
}

/// `link_wallet`'s `UPDATE users_v2 SET wallet = ? WHERE email = ?` has no
/// concept of "this account already has a wallet" — before this fix it would
/// silently re-point an email account from one wallet to another, orphaning
/// whatever KYC/CACF history sat under the old wallet string with nothing to
/// reconcile it. First-time linking must still work; a second link to a
/// DIFFERENT wallet must now be rejected.
#[tokio::test]
async fn link_wallet_rejects_repointing_an_already_linked_account() {
    let app = spawn_app().await;
    let email = "linktest@example.com";
    let password = "correct horse battery staple";

    let (status, body) = app
        .post_json(
            "/api/auth/register-email",
            &json!({ "email": email, "password": password, "username": "linktester" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "register-email: {body}");

    let first_wallet = Keypair::new();
    let first_wallet_pk = first_wallet.pubkey().to_string();
    let ts = now_secs();
    let sig = first_wallet
        .sign_message(format!("xfchess:link:{ts}").as_bytes())
        .to_string();

    let (status, body) = app
        .post_json(
            "/api/auth/link-wallet",
            &json!({
                "email": email, "password": password,
                "wallet": first_wallet_pk, "signature": sig, "timestamp": ts,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "first link should succeed: {body}");

    // Re-linking the SAME wallet again (e.g. a retried request) must still work.
    let ts2 = now_secs();
    let sig2 = first_wallet
        .sign_message(format!("xfchess:link:{ts2}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/link-wallet",
            &json!({
                "email": email, "password": password,
                "wallet": first_wallet_pk, "signature": sig2, "timestamp": ts2,
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "re-linking the same wallet: {body}");

    // Linking a DIFFERENT wallet must now be rejected.
    let second_wallet = Keypair::new();
    let second_wallet_pk = second_wallet.pubkey().to_string();
    let ts3 = now_secs();
    let sig3 = second_wallet
        .sign_message(format!("xfchess:link:{ts3}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/link-wallet",
            &json!({
                "email": email, "password": password,
                "wallet": second_wallet_pk, "signature": sig3, "timestamp": ts3,
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "linking a second, different wallet must be rejected: {body}"
    );

    // Confirm the account still resolves to the FIRST wallet, not the second.
    let (status, body) = app
        .post_json(
            "/api/auth/login",
            &json!({
                "wallet": first_wallet_pk,
                "signature": first_wallet
                    .sign_message(format!("xfchess:login:{}", now_secs()).as_bytes())
                    .to_string(),
                "timestamp": now_secs(),
            }),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the original linked wallet must still be able to log in: {body}"
    );
}

/// `GET /api/auth/lichess/init` used to accept `wallet_pubkey` as a bare
/// query param — only CSRF-protected via the `state` round trip through
/// Lichess, not actually bound to the caller's own wallet. It now requires
/// a Bearer JWT matching `wallet_pubkey`.
#[tokio::test]
async fn lichess_init_requires_auth_and_rejects_wallet_mismatch() {
    let app = spawn_app().await;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let stranger_wallet = Keypair::new().pubkey().to_string();

    // No Authorization header at all.
    let (status, _) = app
        .get(&format!("/api/auth/lichess/init?wallet_pubkey={wallet}"))
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unauthenticated lichess/init must be rejected"
    );

    // Log in as `wallet`.
    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().expect("nonce").to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().expect("token").to_string();

    // A valid JWT for `wallet` trying to init a Lichess link for a
    // DIFFERENT wallet must be rejected.
    let (status, _) = app
        .send_auth(
            "GET",
            &format!("/api/auth/lichess/init?wallet_pubkey={stranger_wallet}"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a valid JWT for one wallet must not init a Lichess link for a different wallet"
    );

    // The authenticated wallet's own request passes the auth gate (fails
    // later with SERVICE_UNAVAILABLE since LICHESS_CLIENT_ID is unset in
    // this test config — proves the auth check specifically, not the whole
    // flow, which needs real Lichess OAuth config to complete).
    let (status, _) = app
        .send_auth(
            "GET",
            &format!("/api/auth/lichess/init?wallet_pubkey={wallet}"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "own-wallet request should clear the auth gate and fail only on missing Lichess config"
    );
}

/// `POST /api/kyc/submit` used to accept `wallet_pubkey` as a bare,
/// unauthenticated field — anyone could submit PII attributed to an
/// arbitrary wallet with zero proof of ownership. It now requires a Bearer
/// JWT and rejects a submission whose body `wallet_pubkey` doesn't match the
/// authenticated wallet, closing that gap the same way `record_move` was
/// fixed earlier for a different route.
#[tokio::test]
async fn kyc_submit_requires_auth_and_rejects_wallet_mismatch() {
    let app = spawn_app().await;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let stranger_wallet = Keypair::new().pubkey().to_string();

    let kyc_body = |wallet_pubkey: &str| {
        json!({
            "wallet_pubkey": wallet_pubkey,
            "country": "GB",
            "full_name": "Test Player",
            "dob": "1990-01-01",
            "residence": "1 Test Street",
            "tax_id": "AB123456C",
        })
    };

    // No Authorization header at all — must be rejected outright.
    let (status, _) = app.post_json("/api/kyc/submit", &kyc_body(&wallet)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unauthenticated KYC submission must be rejected"
    );

    // Log in as `wallet`, then try to submit KYC for a DIFFERENT wallet.
    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().expect("nonce").to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().expect("token").to_string();

    let (status, _) = app
        .send_auth(
            "POST",
            "/api/kyc/submit",
            Some(&token),
            Some(&kyc_body(&stranger_wallet)),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a valid JWT for one wallet must not submit KYC for a different wallet"
    );

    // Submitting KYC for the AUTHENTICATED wallet succeeds.
    let (status, body) = app
        .send_auth(
            "POST",
            "/api/kyc/submit",
            Some(&token),
            Some(&kyc_body(&wallet)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "own-wallet KYC submission: {body}");
}

/// The off-chain (SQLite `users.username`, set via `PATCH /api/auth/username`)
/// and on-chain (`PlayerProfile.username_set`) registration states are
/// completely independent — this is the exact combination that was untested
/// and let a bug ship where a player who'd already chosen a display name
/// off-chain still got asked to "Choose Your Handle" every time they tried to
/// wager, because the game client's profile check (see
/// `src/multiplayer/solana/integration/profile_check.rs`) only ever reads the
/// on-chain side. Both `/api/auth/me` and `/api/auth/sync-profile` gracefully
/// degrade to "no on-chain profile" when the configured RPC URL is
/// unreachable (true in this test config), so this needs no chain mocking.
// multi_thread flavor: `/api/auth/me`'s on-chain existence check runs the
// blocking Solana RpcClient inside `spawn_blocking`, which internally uses
// `tokio::task::block_in_place` — that requires a multi-threaded runtime,
// unlike every other test in this file that never touches `state.solana_rpc`.
#[tokio::test(flavor = "multi_thread")]
async fn offchain_username_does_not_imply_onchain_profile() {
    let app = spawn_app().await;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();

    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().expect("nonce").to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().expect("token").to_string();

    // Set an off-chain handle exactly as the wallet-ui's ProfileStep does on
    // first login (App.tsx's non-on-chain branch).
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "AlreadyRegistered" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "username PATCH should succeed");

    // /auth/me reflects the off-chain name immediately, but must NOT claim an
    // on-chain profile exists — the account-existence RPC call degrades to
    // false against this test's unreachable RPC URL, proving the two
    // registration states are computed independently of each other.
    let (status, body) = app
        .send_auth("GET", "/api/auth/me", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "me: {body}");
    assert_eq!(body["username"], "AlreadyRegistered");
    assert_eq!(
        body["has_onchain_profile"], false,
        "an off-chain username must never be mistaken for an on-chain profile"
    );

    // sync-profile independently confirms: no on-chain PlayerProfile PDA, no
    // on-chain username_set — the exact field the game client's profile check
    // gates the "Choose Your Handle" popup on.
    let (status, body) = app
        .send_auth("POST", "/api/auth/sync-profile", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "sync-profile: {body}");
    assert_eq!(body["has_profile"], false);
    assert_eq!(body["username_set"], false);
}

/// Serialisation guard for the one test that mutates the process-global
/// RELAY_SHARED_SECRET env var. Without this, Cargo's parallel test runner
/// lets other threads see the mutated value → flaky random failures.
static RELAY_TEST_LOCK: Mutex<()> = Mutex::new(());

struct RelaySharedSecretGuard {
    previous: Option<String>,
}

impl RelaySharedSecretGuard {
    fn set(value: &str) -> Self {
        let previous = std::env::var("RELAY_SHARED_SECRET").ok();
        std::env::set_var("RELAY_SHARED_SECRET", value);
        Self { previous }
    }
}

impl Drop for RelaySharedSecretGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var("RELAY_SHARED_SECRET", previous);
        } else {
            std::env::remove_var("RELAY_SHARED_SECRET");
        }
    }
}

/// Dual-accept guard on the signing endpoints: a valid per-user JWT *or* the
/// legacy relay secret is accepted; neither → 401 (fail-closed). JWT callers
/// are also authorized per-wallet on session creation and move submission.
/// All RELAY_SHARED_SECRET handling is kept in this one test to avoid racing
/// the process-global env var.
#[tokio::test]
async fn dual_accept_auth_guards_signing_endpoints() {
    let _guard = RELAY_TEST_LOCK.lock().unwrap();
    let _relay_secret = RelaySharedSecretGuard::set("e2e-relay-secret");
    let app = spawn_app().await;

    let move_body = json!({ "game_id": 1, "move_uci": "e2e4", "next_fen": "x", "nonce": 1 });

    // (a) No auth at all → rejected by the guard.
    let (status, _) = app.post_json("/move/record", &move_body).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no auth must 401");

    // (b) Legacy relay secret alone passes the outer middleware gate (proven
    // via an endpoint with no per-wallet authority, e.g. undelegate_game,
    // which doesn't take `authed`) but is no longer sufficient for
    // record_move: that handler signs as a specific mover_wallet, so it
    // requires a real caller identity (JWT) and rejects relay-secret-only
    // callers itself, even though the middleware let them through.
    let req = Request::builder()
        .uri("/game/undelegate")
        .method("POST")
        .header("content-type", "application/json")
        .header("X-Relay-Secret", "e2e-relay-secret")
        .body(Body::from(
            serde_json::to_vec(&json!({ "game_id": 999999 })).unwrap(),
        ))
        .unwrap();
    let (status, _) = app.send(req).await;
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "valid relay secret must still pass the middleware gate on endpoints with no per-wallet authority"
    );

    let req = Request::builder()
        .uri("/move/record")
        .method("POST")
        .header("content-type", "application/json")
        .header("X-Relay-Secret", "e2e-relay-secret")
        .body(Body::from(serde_json::to_vec(&move_body).unwrap()))
        .unwrap();
    let (status, _) = app.send(req).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "relay-secret-only (no JWT identity) must be rejected by record_move itself — \
         it cannot verify who is submitting the move"
    );

    // (c) Per-user JWT (no relay header) → accepted.
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let token = app.state.jwt.issue(&wallet).expect("issue jwt");
    let (status, _) = app
        .send_auth("POST", "/move/record", Some(&token), Some(&move_body))
        .await;
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "valid JWT must pass the guard"
    );

    // (d) A JWT may only open a session for its own wallet.
    let other = Keypair::new().pubkey().to_string();
    let (status, _) = app
        .send_auth(
            "POST",
            "/session/create",
            Some(&token),
            Some(&json!({ "game_id": 7, "wallet_pubkey": other })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "JWT creating a session for another wallet must 403"
    );

    // (e) Same JWT, own wallet → passes both authn and authz.
    let (status, _) = app
        .send_auth(
            "POST",
            "/session/create",
            Some(&token),
            Some(&json!({ "game_id": 8, "wallet_pubkey": wallet })),
        )
        .await;
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "own-wallet session must pass authn"
    );
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "own-wallet session must pass authz"
    );

    // (f) Secret genuinely unset + no auth at all → the guard fails CLOSED.
    // The game client fetches a JWT automatically on every wallet connection
    // (see `src/multiplayer/network/vps/client.rs`), so an unconfigured
    // relay secret is a fallback, not the primary flow — a missing/invalid
    // credential must never be treated as an unauthenticated pass-through.
    std::env::remove_var("RELAY_SHARED_SECRET");
    let (status, _) = app.post_json("/move/record", &move_body).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "fail-closed: unset secret + no JWT must be rejected by the auth guard"
    );

    // (g) Valid JWT for wallet A, but mover_wallet in the body names wallet B,
    // for a game_id neither wallet is verifiably part of → rejected. A
    // credential authenticating one wallet must not be usable to submit a
    // move for a game it has no on-chain relationship to. (This is NOT the
    // same as "caller must equal mover_wallet" — see
    // `record_move_allows_a_genuine_participant_to_relay_the_other_players_move`
    // in signing::routes::main's own test module for the case this must
    // *allow*: a game's host relays moves for both players, so caller and
    // mover_wallet routinely differ for one side's moves. This test only
    // proves the reject side, since a fake game_id can't be seeded with real
    // on-chain participants from here.)
    let _relay_secret_g = RelaySharedSecretGuard::set("e2e-relay-secret");
    let mismatched_move_body = json!({
        "game_id": 1,
        "move_uci": "e2e4",
        "next_fen": "x",
        "nonce": 1,
        "mover_wallet": other,
    });
    let (status, _) = app
        .send_auth("POST", "/move/record", Some(&token), Some(&mismatched_move_body))
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "JWT for a wallet with no on-chain relationship to this game must 403"
    );
}

#[tokio::test]
async fn admin_route_requires_api_key() {
    let app = spawn_app().await;
    // No X-API-Key header → the require_api_key middleware rejects before the
    // handler runs (so no on-chain path is reached).
    let (status, _) = app
        .post_json(
            "/admin/dispute/resolve",
            &json!({
                "game_id": 1,
                "decision": "DRAW",
                "resolution_text": "n/a",
                "admin_token": "x",
                "white_wallet": "W",
                "black_wallet": "B"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// `treasury_refund` must never sign or submit a transaction itself — see
/// `bin/treasury_signer.rs`'s module doc. It should validate, audit-log, and
/// hand back the CLI command for an operator to run on an isolated host,
/// never a `signature` field (there's nothing to sign with — this process
/// holds no treasury secret key at all, only the public key).
#[tokio::test]
async fn treasury_refund_never_signs_in_process() {
    let app = spawn_app().await;
    let wallet = Keypair::new().pubkey().to_string();

    let req = Request::builder()
        .uri("/admin/treasury/refund")
        .method("POST")
        .header("content-type", "application/json")
        .header("X-API-Key", "dev")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "wallet": wallet,
                "lamports": 1_000_000,
                "reason": "test refund",
                "admin_token": "test-admin-token",
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, body) = app.send(req).await;

    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "awaiting_manual_execution");
    assert!(
        body.get("signature").is_none(),
        "no signature should ever come from this process: {body}"
    );
    let command = body["run_on_isolated_host"]
        .as_str()
        .expect("run_on_isolated_host string");
    assert!(
        command.contains("treasury_signer"),
        "should hand the operator the isolated-host command: {command}"
    );
}
