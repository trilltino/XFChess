//! Tier T2 — live-network RPC smoke tests.
//!
//! `e2e_api.rs` (Tier T1) is deliberately chain-free: it points RPC URLs at an
//! unreachable port so tests stay fast, hermetic, and need no secrets. This
//! file is the opt-in counterpart — it hits whatever `SOLANA_RPC_URL` is
//! actually configured to point at (Triton One in prod/staging; public devnet
//! if you only export a bare URL locally) through the *real* client helpers in
//! `backend::signing::solana::rpc`, including the primary/fallback circuit
//! breaker.
//!
//! Every test is `#[ignore]` so a plain `cargo test` never needs network
//! access or secrets. Run this tier explicitly:
//!
//! ```text
//! just test-rpc-live
//! # or directly:
//! cargo test -p backend --test e2e_rpc_live -- --ignored --nocapture
//! ```
//!
//! after exporting `SOLANA_RPC_URL` (with your Triton x-token embedded in the
//! path, never committed) in the shell or `backend/.env`. If it isn't set,
//! each test prints a skip message and returns rather than failing — running
//! `--ignored` by accident in an environment without the secret configured
//! must stay harmless, not turn into a red build.

use backend::signing::solana::rpc::{fallback_rpc_url, make_rpc, read_with_failover, redact_url};
use solana_sdk::signature::Signer;
use std::str::FromStr;

/// The on-chain program this backend talks to (see `backend/CLAUDE.md`);
/// used as a stable, always-present account to probe read paths against.
const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

fn skip_unless_configured(test_name: &str) -> bool {
    if std::env::var("SOLANA_RPC_URL").is_ok() {
        return false;
    }
    eprintln!(
        "[skip] {test_name}: SOLANA_RPC_URL is not set — export a real RPC URL \
         (Triton devnet endpoint + x-token in prod/staging) to run this tier"
    );
    true
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn primary_rpc_reports_healthy() {
    if skip_unless_configured("primary_rpc_reports_healthy") {
        return;
    }
    let url = backend::signing::solana::rpc::rpc_url_or_devnet();
    println!("[e2e_rpc_live] probing primary RPC: {}", redact_url(&url));
    make_rpc(&url)
        .get_health()
        .expect("primary RPC should report healthy");
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn deployed_program_account_is_reachable_via_failover() {
    if skip_unless_configured("deployed_program_account_is_reachable_via_failover") {
        return;
    }
    let program_id =
        solana_sdk::pubkey::Pubkey::from_str(PROGRAM_ID).expect("valid program id constant");

    // Exercises the real primary/fallback path used in production, not just a
    // raw RpcClient call — this is what `read_with_failover` callers actually get.
    let account = read_with_failover(|rpc| rpc.get_account(&program_id))
        .expect("program account should be readable through the real failover path");
    assert!(
        account.executable,
        "program account must be marked executable on whatever cluster SOLANA_RPC_URL points at"
    );
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn fallback_url_is_independently_reachable() {
    if skip_unless_configured("fallback_url_is_independently_reachable") {
        return;
    }
    // Confirms the fallback endpoint is itself alive, so a real primary outage
    // would actually have somewhere to fail over to.
    let fb = fallback_rpc_url();
    println!("[e2e_rpc_live] probing fallback RPC: {}", redact_url(&fb));
    make_rpc(&fb)
        .get_health()
        .expect("fallback RPC should report healthy");
}

// ── Full on-chain profile-creation loop (opt-in, spends real devnet SOL) ────
//
// Exercises the exact gap Tier T1 (e2e_api.rs) can't reach because it's
// chain-free by design: does completing the wallet-ui's ProfileStep on-chain
// branch (tauri/wallet-ui/src/App.tsx's `requireOnchain` path) actually flip
// `PlayerProfile.username_set` on a real cluster? Spins up the real app
// router (same construction as e2e_api.rs's spawn_app, but pointed at the
// real SOLANA_RPC_URL instead of an unreachable port), signs with a
// throwaway devnet keypair instead of a wallet extension — the wire format
// (bincode-serialized legacy Transaction, base64) is identical either way —
// and polls sync-profile for the flip.

async fn spawn_live_app(rpc_url: &str) -> axum::Router {
    use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
    use backend::signing::storage::tournament::TournamentStore;
    use backend::signing::storage::SessionStore;
    use backend::signing::{AppState, SigningConfig};

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let db_url =
        |tag: &str| format!("sqlite:file:xfchess_e2e_live_{tag}_{nanos}?mode=memory&cache=shared");

    let pools = initialize_pools(&db_url("session"), &db_url("vault"))
        .await
        .expect("init pools");
    run_migrations(&pools).await.expect("run migrations");
    let session_store = SessionStore::new(pools.session_pool.clone());
    session_store.init().await.expect("session store init");
    let tournament_store = TournamentStore::new(pools.session_pool.clone()).await;

    let config = SigningConfig {
        port: 0,
        solana_rpc_url: rpc_url.to_string(),
        solana_mainnet_rpc_url: None,
        er_rpc_url: rpc_url.to_string(),
        magic_router_rpc_url: rpc_url.to_string(),
        program_id: PROGRAM_ID.to_string(),
        jwt_secret: "test-secret-not-for-production".into(),
        identity_encryption_key: "0".repeat(64),
        identity_salt: "0".repeat(64),
        fee_payer_keys: vec![],
        vps_authority_key: None,
        kyc_authority_key: None,
        link_authority_key: None,
        treasury_authority_key: None,
        admin_token: Some("test-admin-token".into()),
        tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
        usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
        lichess_client_id: String::new(),
    };
    let state = AppState::new(
        config,
        pools.session_pool.clone(),
        pools.vault_pool.clone(),
        std::sync::Arc::new(tournament_store),
    );
    let _ = state.friends.init().await;
    build_app_router(state.clone()).with_state(state)
}

async fn live_send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<&serde_json::Value>,
) -> (axum::http::StatusCode, serde_json::Value) {
    use tower::ServiceExt;
    let mut b = axum::http::Request::builder().uri(uri).method(method);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) if !v.is_null() => b
            .header("content-type", "application/json")
            .body(axum::body::Body::from(serde_json::to_vec(v).unwrap()))
            .unwrap(),
        _ => b.body(axum::body::Body::empty()).unwrap(),
    };
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

#[tokio::test]
#[ignore = "hits a live RPC endpoint and spends real devnet SOL (airdrop) — opt in with --ignored"]
async fn init_profile_flow_flips_onchain_username_set() {
    if skip_unless_configured("init_profile_flow_flips_onchain_username_set") {
        return;
    }
    let rpc_url = std::env::var("SOLANA_RPC_URL").expect("checked by skip_unless_configured");
    let router = spawn_live_app(&rpc_url).await;

    // Throwaway devnet identity — never reused outside this test.
    let kp = solana_sdk::signature::Keypair::new();
    let wallet = kp.pubkey().to_string();

    // Airdrop enough devnet SOL to cover both PDAs' rent (~0.005 SOL total).
    let rpc = solana_client::rpc_client::RpcClient::new(rpc_url.clone());
    let airdrop_sig = rpc
        .request_airdrop(&kp.pubkey(), 1_000_000_000)
        .expect("devnet airdrop request");
    let blockhash = rpc
        .get_latest_blockhash()
        .expect("blockhash for airdrop confirm");
    rpc.confirm_transaction_with_spinner(
        &airdrop_sig,
        &blockhash,
        solana_commitment_config::CommitmentConfig::confirmed(),
    )
    .expect("airdrop should confirm");

    // SIWS login → JWT (same flow as e2e_api.rs's siws_login_then_logout_revokes_token).
    let (status, body) = live_send(
        &router,
        "POST",
        "/api/auth/siws-challenge",
        None,
        Some(&serde_json::json!({ "wallet": wallet })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().expect("nonce").to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = live_send(
        &router,
        "POST",
        "/api/auth/siws-verify",
        None,
        Some(&serde_json::json!({ "wallet": wallet, "signature": sig, "nonce": nonce })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().expect("token").to_string();

    // Build the on-chain init_profile tx exactly as ProfileStep's
    // requireOnchain branch does (tauri/wallet-ui/src/App.tsx).
    let date_of_birth = chrono::Utc::now().timestamp() - 700_000_000; // well over 18 years old
    let (status, body) = live_send(
        &router,
        "POST",
        "/api/auth/init-profile-tx",
        Some(&token),
        Some(&serde_json::json!({
            "username": "E2eDevnetPlayer",
            "country": "GB",
            "date_of_birth": date_of_birth,
        })),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "init-profile-tx: {body}"
    );
    let tx_b64 = body["tx_b64"].as_str().expect("tx_b64").to_string();

    // Sign locally with the throwaway keypair — this is exactly what
    // provider.signTransaction(tx) does in the real wallet-ui flow.
    let tx_bytes = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&tx_b64)
            .expect("base64 decode tx")
    };
    let mut tx: solana_sdk::transaction::Transaction =
        bincode::deserialize(&tx_bytes).expect("deserialize tx");
    let recent_blockhash = tx.message.recent_blockhash;
    tx.try_sign(&[&kp], recent_blockhash).expect("sign tx");
    let signed_bytes = bincode::serialize(&tx).expect("serialize signed tx");
    let signed_b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&signed_bytes)
    };

    let (status, body) = live_send(
        &router,
        "POST",
        "/api/auth/broadcast-tx",
        None,
        Some(&serde_json::json!({ "tx_b64": signed_b64 })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "broadcast-tx: {body}");

    // Poll sync-profile until the on-chain flip is visible (devnet confirmation lag).
    let mut username_set = false;
    for _ in 0..10 {
        let (status, body) = live_send(
            &router,
            "POST",
            "/api/auth/sync-profile",
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "sync-profile: {body}");
        if body["username_set"] == true {
            username_set = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    assert!(
        username_set,
        "on-chain PlayerProfile.username_set should flip to true after broadcast-tx"
    );
}

// ── Admin tournament creation → game-client discovery (opt-in, real devnet) ─
//
// `POST /admin/tournament/create` (the route the tournament-admin panel's
// CreateTournament wizard actually calls) is normally only exercised
// end-to-end via a dev binary calling the on-chain program directly — this
// route's own wiring (its `vps_authority` signer, its retry/idempotency
// logic) had no test at all. Requires a *funded* `VPS_AUTHORITY_KEY` in the
// environment — if unset, the backend falls back to a random unfunded key
// and every on-chain init transaction below fails with a clear RPC error
// (see `backend/src/signing/mod.rs`), which is exactly the real-world
// failure mode this test is meant to catch early.
#[tokio::test]
#[ignore = "hits a live RPC endpoint and spends real devnet SOL (3 on-chain inits) — opt in with --ignored"]
async fn admin_creates_tournament_and_game_client_can_see_it() {
    if skip_unless_configured("admin_creates_tournament_and_game_client_can_see_it") {
        return;
    }
    let rpc_url = std::env::var("SOLANA_RPC_URL").expect("checked by skip_unless_configured");
    let router = spawn_live_app(&rpc_url).await;

    // Same fallback the middleware itself uses in debug builds (cargo test
    // always builds with debug_assertions on), so this works whether or not
    // the environment sets a custom ADMIN_API_KEY.
    let api_key = std::env::var("ADMIN_API_KEY").unwrap_or_else(|_| "dev".to_string());

    // Unique per run so repeated manual runs never collide on "tournament_id
    // already in use".
    let tournament_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);

    let create_body = serde_json::json!({
        "tournament_id": tournament_id,
        "name": "E2E Devnet Tournament",
        "entry_fee_lamports": 10_000,
        "platform_fee_lamports": 0,
        "max_players": 2,
        "format": "SingleElimination",
    });

    let req = axum::http::Request::builder()
        .uri("/admin/tournament/create")
        .method("POST")
        .header("content-type", "application/json")
        .header("X-API-Key", &api_key)
        .body(axum::body::Body::from(
            serde_json::to_vec(&create_body).unwrap(),
        ))
        .unwrap();
    let resp = {
        use tower::ServiceExt;
        router.clone().oneshot(req).await.unwrap()
    };
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "admin/tournament/create: {body}"
    );
    assert_eq!(body["tournament_id"], tournament_id);

    // The exact endpoint the game client's poll_tournament_list hits
    // (src/multiplayer/solana/tournament.rs) — proves creation and
    // discovery go through the same store, not just that creation alone
    // returned 200.
    let (status, body) = live_send(&router, "GET", "/tournaments", None, None).await;
    assert_eq!(status, axum::http::StatusCode::OK, "tournaments: {body}");
    let listed = body
        .as_array()
        .expect("tournaments list")
        .iter()
        .any(|t| t["tournament_id"] == tournament_id);
    assert!(
        listed,
        "newly created tournament {tournament_id} should appear in GET /tournaments"
    );
}
