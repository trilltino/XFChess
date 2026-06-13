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
use backend::signing::storage::tournament::TournamentStore;
use backend::signing::storage::SessionStore;
use backend::signing::{AppState, SigningConfig};

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
        er_rpc_url: "http://127.0.0.1:9".into(),
        program_id: "AhkTK5LVJHvR51gmDXbsJsqq4wg381AH6vTiaFGGJPWm".into(),
        jwt_secret: "test-secret-not-for-production".into(),
        identity_encryption_key: "0".repeat(64),
        identity_salt: "0".repeat(64),
        fee_payer_keys: vec![],
        vps_authority_key: None,
        kyc_authority_key: None,
        link_authority_key: None,
        admin_token: Some("test-admin-token".into()),
        host_treasury_pubkey: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
        usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
        lichess_client_id: String::new(),
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

    let session_store = SessionStore::new(pools.session_pool.clone());
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
    assert!(body.contains("xfchess_settlement_ticks_total"), "missing settlement metric:\n{body}");
    assert!(body.contains("xfchess_anticheat_queue_depth"), "missing anticheat metric");
    assert!(body.contains("xfchess_linkage_flagged_total"), "missing linkage metric");
    assert!(body.contains("xfchess_prize_distribution_held_total"), "missing prize metric");
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
    repo.add_move_simple(game, 1, "e2e4", None, Some("fen1"), "white").await.unwrap();
    repo.add_move_simple(game, 2, "e7e5", None, Some("fen2"), "black").await.unwrap();

    let (status, body) = app.get(&format!("/games/moves/{game}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["moves"].as_array().unwrap().len(), 2, "live feed shows all moves");

    // Apply a 1-hour delay: the just-recorded moves are inside the window and
    // must disappear from the public feed.
    repo.set_broadcast_delay(game, 3600).await.unwrap();
    let (status, body) = app.get(&format!("/games/moves/{game}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["moves"].as_array().unwrap().len(), 0, "delayed feed withholds recent moves");

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
