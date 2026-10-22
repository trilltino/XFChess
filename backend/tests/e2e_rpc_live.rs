use anyhow::Context as _;
use backend::signing::solana::rpc::{fallback_rpc_url, make_rpc, read_with_failover, redact_url};
use solana_sdk::signature::Signer;
use std::str::FromStr;

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
fn primary_rpc_reports_healthy() -> anyhow::Result<()> {
    if skip_unless_configured("primary_rpc_reports_healthy") {
        return Ok(());
    }
    let url = backend::signing::solana::rpc::rpc_url_or_devnet();
    println!("[e2e_rpc_live] probing primary RPC: {}", redact_url(&url));
    make_rpc(&url)?
        .get_health()
        .context("primary RPC should report healthy")?;

    Ok(())
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn deployed_program_account_is_reachable_via_failover() -> anyhow::Result<()> {
    if skip_unless_configured("deployed_program_account_is_reachable_via_failover") {
        return Ok(());
    }
    let program_id =
        solana_sdk::pubkey::Pubkey::from_str(PROGRAM_ID).context("valid program id constant")?;

    // Exercises the real primary/fallback path used in production, not just a
    // raw RpcClient call — this is what `read_with_failover` callers actually get.
    let account = read_with_failover(|rpc| rpc.get_account(&program_id))
        .context("program account should be readable through the real failover path")?;
    assert!(
        account.executable,
        "program account must be marked executable on whatever cluster SOLANA_RPC_URL points at"
    );

    Ok(())
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn fallback_url_is_independently_reachable() -> anyhow::Result<()> {
    if skip_unless_configured("fallback_url_is_independently_reachable") {
        return Ok(());
    }
    // Confirms the fallback endpoint is itself alive, so a real primary outage
    // would actually have somewhere to fail over to.
    let fb = fallback_rpc_url();
    println!("[e2e_rpc_live] probing fallback RPC: {}", redact_url(&fb));
    make_rpc(&fb)?
        .get_health()
        .context("fallback RPC should report healthy")?;

    Ok(())
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn debug_transaction_reports_real_on_chain_outcome() -> anyhow::Result<()> {
    if skip_unless_configured("debug_transaction_reports_real_on_chain_outcome") {
        return Ok(());
    }
    // Pin a real finalized devnet program transaction; update it after a devnet reset.
    const KNOWN_GOOD_SIG: &str =
        "2riEGnPfQQcvWRdnSDnW1kNMLU5TtpYE1CktsrLz9gssbrNZ1VyjwSXQk55fwUqQPiN5NQprsLU2A2Yxgv9pVDQM";

    let url = backend::signing::solana::rpc::rpc_url_or_devnet();
    let rpc = make_rpc(&url)?;
    let sig = solana_sdk::signature::Signature::from_str(KNOWN_GOOD_SIG)
        .context("valid signature constant")?;

    let info = tokio::runtime::Runtime::new()
        .context("test setup failed")?
        .block_on(backend::signing::solana::debug::debug_transaction(
            &rpc, &sig,
        ))
        .context("known-good signature should be found on-chain")?;

    assert!(
        info.success,
        "known-good tx should report success: {info:?}"
    );
    assert!(
        info.slot > 0,
        "slot should be populated from the real transaction"
    );
    assert_eq!(info.signature, KNOWN_GOOD_SIG);
    assert!(
        info.program_ids.contains(&PROGRAM_ID.to_string()),
        "program_ids should include this backend's own program: {:?}",
        info.program_ids
    );

    Ok(())
}

#[test]
#[ignore = "hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored"]
fn debug_transaction_reports_real_on_chain_failure() -> anyhow::Result<()> {
    if skip_unless_configured("debug_transaction_reports_real_on_chain_failure") {
        return Ok(());
    }
    // Pin a finalized failed transaction to verify success detection. Re-pin after devnet resets.
    const KNOWN_FAILED_SIG: &str =
        "3tzUrvpK9ViBwc4fuHgLZ3fJGmiRjotF416dxrtgXEMYBfpiH4dukRk8G3jXMvzsBW72Gp8Y6JE9UeP3F9UWfNa2";

    let url = backend::signing::solana::rpc::rpc_url_or_devnet();
    let rpc = make_rpc(&url)?;
    let sig = solana_sdk::signature::Signature::from_str(KNOWN_FAILED_SIG)
        .context("valid signature constant")?;

    let info = tokio::runtime::Runtime::new()
        .context("test setup failed")?
        .block_on(backend::signing::solana::debug::debug_transaction(
            &rpc, &sig,
        ))
        .context("known-failed signature should still be found on-chain")?;

    assert!(
        !info.success,
        "known-failed tx must report success: false, not the old hardcoded true: {info:?}"
    );
    assert!(
        info.error.is_some(),
        "a failed transaction should carry a real error string"
    );

    Ok(())
}

// Opt-in devnet profile-creation test using a throwaway signer and the
// wallet UI wire format. Verify that sync-profile observes username_set.

async fn spawn_live_app(rpc_url: &str) -> anyhow::Result<axum::Router> {
    Ok({
        use backend::infrastructure::{build_app_router, initialize_pools, run_migrations};
        use backend::signing::storage::tournament::TournamentStore;
        use backend::signing::{AppState, SigningConfig};

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let db_url = |tag: &str| {
            format!("sqlite:file:xfchess_e2e_live_{tag}_{nanos}?mode=memory&cache=shared")
        };

        let pools = initialize_pools(&db_url("session"), &db_url("vault"))
            .await
            .context("init pools")?;
        run_migrations(&pools).await.context("run migrations")?;
        let tournament_store = TournamentStore::new(pools.session_pool.clone()).await?;

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
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
            admin_token: Some("test-admin-token".into()),
            tournament_fee_recipient: "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C".into(),
            usdc_mint_pubkey: "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".into(),
            lichess_client_id: String::new(),
            allowed_origins: vec![],
        };
        let state = AppState::new(
            config,
            pools.session_pool.clone(),
            pools.vault_pool.clone(),
            std::sync::Arc::new(tournament_store),
        )?;
        build_app_router(state.clone()).with_state(state)
    })
}

async fn live_send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<&serde_json::Value>,
) -> anyhow::Result<(axum::http::StatusCode, serde_json::Value)> {
    Ok({
        use tower::ServiceExt;
        let mut b = axum::http::Request::builder().uri(uri).method(method);
        if let Some(t) = bearer {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(v) if !v.is_null() => b
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(v).context("test setup failed")?,
                ))
                .context("test setup failed")?,
            _ => b
                .body(axum::body::Body::empty())
                .context("test setup failed")?,
        };
        let resp = router
            .clone()
            .oneshot(req)
            .await
            .context("test setup failed")?;
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .context("test setup failed")?;
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, value)
    })
}

#[tokio::test]
#[ignore = "hits a live RPC endpoint and spends real devnet SOL (airdrop) — opt in with --ignored"]
async fn init_profile_flow_flips_onchain_username_set() -> anyhow::Result<()> {
    if skip_unless_configured("init_profile_flow_flips_onchain_username_set") {
        return Ok(());
    }
    let rpc_url = std::env::var("SOLANA_RPC_URL").context("checked by skip_unless_configured")?;
    let router = spawn_live_app(&rpc_url).await?;

    // Throwaway devnet identity — never reused outside this test.
    let kp = solana_sdk::signature::Keypair::new();
    let wallet = kp.pubkey().to_string();

    // Airdrop enough devnet SOL to cover both PDAs' rent (~0.005 SOL total).
    let rpc = solana_client::rpc_client::RpcClient::new(rpc_url.clone());
    let airdrop_sig = rpc
        .request_airdrop(&kp.pubkey(), 1_000_000_000)
        .context("devnet airdrop request")?;
    let blockhash = rpc
        .get_latest_blockhash()
        .context("blockhash for airdrop confirm")?;
    rpc.confirm_transaction_with_spinner(
        &airdrop_sig,
        &blockhash,
        solana_commitment_config::CommitmentConfig::confirmed(),
    )
    .context("airdrop should confirm")?;

    // SIWS login → JWT (same flow as e2e_api.rs's siws_login_then_logout_revokes_token).
    let (status, body) = live_send(
        &router,
        "POST",
        "/api/auth/siws-challenge",
        None,
        Some(&serde_json::json!({ "wallet": wallet })),
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().context("nonce")?.to_string();
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
    .await?;
    assert_eq!(status, axum::http::StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().context("token")?.to_string();

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
    .await?;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "init-profile-tx: {body}"
    );
    let tx_b64 = body["tx_b64"].as_str().context("tx_b64")?.to_string();

    // Sign locally with the throwaway keypair — this is exactly what
    // provider.signTransaction(tx) does in the real wallet-ui flow.
    let tx_bytes = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&tx_b64)
            .context("base64 decode tx")?
    };
    let mut tx: solana_sdk::transaction::Transaction =
        bincode::deserialize(&tx_bytes).context("deserialize tx")?;
    let recent_blockhash = tx.message.recent_blockhash;
    tx.try_sign(&[&kp], recent_blockhash).context("sign tx")?;
    let signed_bytes = bincode::serialize(&tx).context("serialize signed tx")?;
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
    .await?;
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
        .await?;
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

    Ok(())
}

// Opt-in devnet tournament creation and discovery test. Requires a funded
// VPS_AUTHORITY_KEY for the real admin initialization transactions.
#[tokio::test]
#[ignore = "hits a live RPC endpoint and spends real devnet SOL (3 on-chain inits) — opt in with --ignored"]
async fn admin_creates_tournament_and_game_client_can_see_it() -> anyhow::Result<()> {
    if skip_unless_configured("admin_creates_tournament_and_game_client_can_see_it") {
        return Ok(());
    }
    let rpc_url = std::env::var("SOLANA_RPC_URL").context("checked by skip_unless_configured")?;
    let router = spawn_live_app(&rpc_url).await?;

    // Match the middleware’s debug API-key fallback while allowing environment overrides.
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
            serde_json::to_vec(&create_body).context("test setup failed")?,
        ))
        .context("test setup failed")?;
    let resp = {
        use tower::ServiceExt;
        router
            .clone()
            .oneshot(req)
            .await
            .context("test setup failed")?
    };
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("test setup failed")?;
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "admin/tournament/create: {body}"
    );
    assert_eq!(body["tournament_id"], tournament_id);

    // Verify the game client's discovery endpoint sees the newly created tournament.
    let (status, body) = live_send(&router, "GET", "/tournaments", None, None).await?;
    assert_eq!(status, axum::http::StatusCode::OK, "tournaments: {body}");
    let listed = body
        .as_array()
        .context("tournaments list")?
        .iter()
        .any(|t| t["tournament_id"] == tournament_id);
    assert!(
        listed,
        "newly created tournament {tournament_id} should appear in GET /tournaments"
    );

    Ok(())
}
