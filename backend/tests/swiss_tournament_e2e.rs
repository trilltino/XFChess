//! End-to-end Swiss tournament lifecycle test, driven through the real HTTP
//! router (`build_app_router`) and the real `SwissService`/`TournamentStore`
//! (Tier T1; see `tests/e2e_api.rs`'s module doc for the pattern this follows).
//!
//! Tournament creation is seeded directly into `TournamentStore` rather than
//! through `POST /admin/tournament/create` — that route also submits three
//! on-chain PDA-setup transactions (see `create_tournament` in
//! `signing/routes/tournament.rs`), which this in-process test has no RPC to
//! satisfy. Registration (`/tournament/{id}/join`), Swiss initialization,
//! pairing generation, result recording, round advancement, and standings are
//! all chain-free and are exercised for real over HTTP.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
use backend::signing::identity::IdentityVault;
use backend::signing::storage::tournament::{TournamentFormat, TournamentRecord, TournamentStore};
use backend::signing::storage::SessionStore;
use backend::signing::{AppState, SigningConfig};

fn unique_db_url(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sqlite:file:xfchess_swiss_e2e_{tag}_{n}_{nanos}?mode=memory&cache=shared")
}

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

    /// Same as `post_json`, plus the `X-API-Key` header `require_api_key`
    /// checks on admin routes. No `ADMIN_API_KEY` env var is set anywhere in
    /// this file, so the middleware's debug-build default (`"dev"`) applies —
    /// deliberately avoided setting the process-global env var to sidestep
    /// the cross-test race `tests/e2e_api.rs` documents for the same reason.
    async fn post_json_admin(&self, uri: &str, body: &Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .method("POST")
            .header("content-type", "application/json")
            .header("X-API-Key", "dev")
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
}

async fn spawn_app() -> TestApp {
    let pools = initialize_pools(&unique_db_url("session"), &unique_db_url("vault"))
        .await
        .expect("init pools");
    run_migrations(&pools).await.expect("run migrations");

    // Only used to create/migrate tables via `.init()` — AppState::new below
    // builds the real store this test actually reads/writes through.
    let schema_vault = IdentityVault::new(&"0".repeat(64), &"0".repeat(64)).expect("test vault");
    let session_store = SessionStore::new(pools.session_pool.clone(), schema_vault);
    session_store.init().await.expect("session store init");

    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

    let state = AppState::new(
        test_config(),
        pools.session_pool.clone(),
        pools.vault_pool.clone(),
        Arc::new(tournament_store),
    );

    TestApp { state }
}

/// Full 8-player, 5-round Swiss tournament through the real HTTP surface:
/// join -> initialize -> round 1 pairings -> record results -> auto-advance
/// to round 2 -> round 2 pairings -> standings.
#[tokio::test]
async fn swiss_tournament_full_lifecycle_via_http() {
    let app = spawn_app().await;
    let tournament_id = 9001_u64;

    let players: [(&str, u32); 8] = [
        ("gm_wallet", 2600),
        ("m1_wallet", 2400),
        ("m2_wallet", 2300),
        ("e1_wallet", 2100),
        ("e2_wallet", 2000),
        ("c1_wallet", 1600),
        ("c2_wallet", 1500),
        ("novice_wallet", 1200),
    ];

    // Seed the tournament directly into the store (see module doc: this
    // skips the on-chain-coupled `POST /admin/tournament/create`).
    let record = TournamentRecord::with_config(
        tournament_id,
        "Test Swiss Tournament".to_string(),
        1_000_000,
        0,
        8,
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
        TournamentFormat::Swiss { rounds: 5 },
        None,
        None,
        Some(8),
        None,
        false,
    );
    app.state.tournament_store.create(record).await;

    // Join all 8 players over real HTTP (chain-free route).
    for (wallet, elo) in players {
        let (status, body) = app
            .post_json(
                &format!("/tournament/{tournament_id}/join"),
                &json!({ "player": wallet, "elo": elo }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "join {wallet}: {body}");
        assert_eq!(body["ok"], json!(true), "join {wallet}: {body}");
    }

    // Initialize Swiss: seeds by ELO, marks Active, starts round 1.
    let (status, body) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/initialize-swiss"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "initialize-swiss: {body}");

    let (status, tournament) = app.get(&format!("/tournament/{tournament_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tournament["status"], json!("Active"));
    assert_eq!(tournament["players"].as_array().unwrap().len(), 8);

    let (status, round) = app
        .get(&format!("/tournament/{tournament_id}/current-round"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(round["round"], json!(1));
    assert_eq!(round["total_rounds"], json!(5));
    assert_eq!(round["is_active"], json!(true));

    let (status, my_match) = app
        .get(&format!(
            "/tournament/{tournament_id}/my-match?player=gm_wallet"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(my_match["found"], json!(true));
    assert_eq!(my_match["round"], json!(1));
    assert!(my_match["board"].is_number());

    // Round 1 pairings: 4 boards, no self-pairings.
    let (status, pairings_r1) = app
        .get(&format!("/tournament/{tournament_id}/pairings/1"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let boards_r1 = pairings_r1["pairings"].as_array().unwrap().clone();
    assert_eq!(boards_r1.len(), 4, "round 1: {pairings_r1}");
    for board in &boards_r1 {
        assert_ne!(board["white"], board["black"], "self-pairing: {board}");
    }

    // Find GrandMaster's own board/color straight from the pairing data that
    // `record_result` actually reads (rather than `/my-match`'s derived
    // `your_color`, which is computed separately and isn't guaranteed to
    // agree on which side is which).
    let gm_pairing = boards_r1
        .iter()
        .find(|b| b["white"] == json!("gm_wallet") || b["black"] == json!("gm_wallet"))
        .expect("GrandMaster should have a round 1 pairing");
    let gm_board = gm_pairing["board"].as_u64().unwrap();
    let gm_is_white = gm_pairing["white"] == json!("gm_wallet");

    // Record round 1: GrandMaster wins decisively on their own board, every
    // other board draws. This makes GrandMaster the sole 1.0-point leader
    // with no standings tiebreak ambiguity for the assertion below.
    for board in &boards_r1 {
        let board_num = board["board"].as_u64().unwrap();
        let result = if board_num == gm_board {
            if gm_is_white {
                "1-0"
            } else {
                "0-1"
            }
        } else {
            "0.5-0.5"
        };
        let (status, body) = app
            .post_json_admin(
                &format!("/admin/tournament/{tournament_id}/result"),
                &json!({ "round": 1, "board": board_num, "result": result }),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "record result board {board_num}: {body}"
        );
    }

    // All 4 results recorded -> the service auto-starts round 2.
    let (status, round) = app
        .get(&format!("/tournament/{tournament_id}/current-round"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(round["round"], json!(2), "should auto-advance to round 2");

    let (status, pairings_r2) = app
        .get(&format!("/tournament/{tournament_id}/pairings/2"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let boards_r2 = pairings_r2["pairings"].as_array().unwrap();
    assert!(!boards_r2.is_empty(), "round 2 should have pairings");
    assert!(
        boards_r2.len() <= boards_r1.len(),
        "Swiss rounds should not create more pairings than the previous round"
    );
    for board in boards_r2 {
        assert_ne!(board["white"], board["black"], "self-pairing: {board}");
    }

    // Standings reflect round 1 only (round 2 has no results yet): sorted
    // descending by score, GrandMaster alone at the top.
    let (status, standings) = app
        .get(&format!("/tournament/{tournament_id}/standings"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let standings = standings.as_array().unwrap();
    assert_eq!(standings.len(), 8);
    for pair in standings.windows(2) {
        let a = pair[0]["score"].as_f64().unwrap();
        let b = pair[1]["score"].as_f64().unwrap();
        assert!(a >= b, "standings should be sorted by score: {a} then {b}");
    }
    assert_eq!(standings[0]["player_id"], json!("gm_wallet"));
    assert_eq!(standings[0]["score"], json!(1.0));
}

/// `POST /admin/tournament/{id}/approve-prize-release` is the human-in-the-loop
/// gate `spawn_prize_distributor` checks before paying out a prize pool above
/// `PRIZE_AUTO_RELEASE_THRESHOLD_LAMPORTS` — see
/// `tasks::tournament_scheduler::awaiting_prize_release_approval`. This test
/// only exercises the HTTP surface (auth + persistence); the distributor's
/// own gating logic has direct unit tests in that module.
#[tokio::test]
async fn approve_prize_release_route_sets_the_flag() {
    let app = spawn_app().await;
    let tournament_id = 9100_u64;

    let record = TournamentRecord::new(tournament_id, "Big Prize Tournament", 0);
    app.state.tournament_store.create(record).await;

    let fetched = app
        .state
        .tournament_store
        .get(tournament_id)
        .await
        .expect("seeded tournament");
    assert!(!fetched.prize_release_approved, "should start unapproved");

    // No X-API-Key → rejected before the handler runs, same as every other
    // admin route.
    let (status, _) = app
        .post_json(
            &format!("/admin/tournament/{tournament_id}/approve-prize-release"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        !app.state
            .tournament_store
            .get(tournament_id)
            .await
            .unwrap()
            .prize_release_approved,
        "unauthorized call must not have flipped the flag"
    );

    // With the admin key → approved.
    let (status, body) = app
        .post_json_admin(
            &format!("/admin/tournament/{tournament_id}/approve-prize-release"),
            &json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        app.state
            .tournament_store
            .get(tournament_id)
            .await
            .unwrap()
            .prize_release_approved,
        "approved flag should now be set"
    );

    // Unknown tournament → 404, not a silent success.
    let (status, _) = app
        .post_json_admin("/admin/tournament/999999/approve-prize-release", &json!({}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
