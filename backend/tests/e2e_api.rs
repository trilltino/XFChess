mod common;
use anyhow::Context as _;
use common::{spawn_app, spawn_app_on, unique_db_url, TestApp};

use axum::body::Body;
use axum::http::{Request, StatusCode};

use serde_json::{json, Value};


use tokio::sync::Mutex;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cu_benchmark_runs_100_games_through_backend() -> anyhow::Result<()> {
    use cu_benchmark::system::{report, runner, scenario::Scenario};
    use solana_sdk::signature::{Keypair, Signer};

    let databases = tempfile::tempdir().context("test setup failed")?;
    let session_url = format!(
        "sqlite:{}?mode=rwc",
        databases.path().join("session.db").display()
    );
    let vault_url = format!(
        "sqlite:{}?mode=rwc",
        databases.path().join("vault.db").display()
    );
    let app = spawn_app_on(&session_url, &vault_url).await?;
    let white = Keypair::new();
    let black = Keypair::new();
    for (key, name) in [(&white, "bench-white"), (&black, "bench-black")] {
        app.state
            .store
            .create_wallet_user(&key.pubkey().to_string(), name, None)
            .await
            .context("test setup failed")?;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("test setup failed")?;
    let base = format!(
        "http://{}",
        listener.local_addr().context("test setup failed")?
    );
    let router = app.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let mut config: Value = serde_json::from_str(include_str!(
        "../../tools/cu_benchmark/scenarios/backend.json"
    ))
    .context("test setup failed")?;
    config["backend_url"] = json!(base);
    config["stages"] = json!([{"name":"100-games","games":100,"concurrency":10}]);
    config["variables"] = json!({"white_wallet":white.pubkey().to_string(),"black_wallet":black.pubkey().to_string()});
    let timestamp = chrono::Utc::now().timestamp() as u64;
    let message = format!("xfchess:login:{timestamp}");
    let mut login = Vec::new();
    for (name, key, capture) in [
        ("login-white", &white, "white_token"),
        ("login-black", &black, "black_token"),
    ] {
        login.push(json!({"name":name,"method":"POST","path":"/api/auth/login",
            "body":{"wallet":key.pubkey().to_string(),"signature":key.sign_message(message.as_bytes()).to_string(),"timestamp":timestamp},
            "capture":{capture:"/token"},"expect":{"/wallet":key.pubkey().to_string()}}));
    }
    login.extend(
        config["workflow"]["setup"]
            .as_array()
            .context("test setup failed")?
            .clone(),
    );
    config["workflow"]["setup"] = json!(login);
    config["workflow"]["move_interval_ms"] = json!(10);
    config["sampling_ms"] = json!(50);
    config["baseline_ms"] = json!(100);
    config["drain_ms"] = json!(100);
    config["limits"] = json!({"request_p95_ms":10000,"error_ratio":0.1,"min_duration_ms":0,"max_schedule_lag_ms":10000});
    let telemetry = config["telemetry"]
        .as_array_mut()
        .context("test setup failed")?;
    telemetry.retain(|metric| {
        !metric["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("host-"))
            && metric["name"] != "egress"
    });
    for metric in telemetry {
        metric["url"] = json!(format!("{base}/metrics"));
    }
    config["pricing"] = json!({"configured":true,"currency":"GBP","monthly_games":10000,
        "fixed":[{"name":"fixture-only VPS assumption","monthly_minor_units":2000}],
        "metered":[{"name":"fixture-only RPC assumption","metric":"rpc-calls","quantity_per_unit":1000,"minor_units_per_unit":1}]});
    let scenario: Scenario = serde_json::from_value(config).context("test setup failed")?;
    scenario.validate().context("test setup failed")?;
    let observed = runner::execute(&scenario, false)
        .await
        .context("test setup failed")?;
    let report = report::build(&scenario, observed).context("test setup failed")?;
    let output = std::env::temp_dir()
        .join("xfchess")
        .join("cu_benchmark-backend-e2e.json");
    std::fs::create_dir_all(output.parent().context("test setup failed")?)
        .context("test setup failed")?;
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(&report).context("test setup failed")?,
    )
    .context("test setup failed")?;
    let stage = &report.stages[0];
    assert_eq!(stage.observed.completed_games, 100, "{:?}", stage.reasons);
    assert_eq!(stage.observed.cleanup_failed_games, 0);
    assert_eq!(stage.observed.streams.connections, 200);
    assert_eq!(stage.observed.streams.errors, 0);
    assert!(stage.observed.streams.received_chunks >= 200);
    assert!(stage.metrics["backend-memory"].complete);
    assert!(stage.metrics["rpc-calls"].complete);
    assert_eq!(stage.operating_costs.fixed_monthly_minor_units, Some(2000));
    assert_eq!(stage.measured_transaction_fees_lamports, 0);
    let moves: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_event_log WHERE kind = 'move'")
        .fetch_one(&app.state.store.pool())
        .await
        .context("test setup failed")?;
    assert_eq!(moves, 400);
    assert_eq!(
        report.max_passing_observed_concurrency,
        Some(10),
        "{:?}",
        stage.reasons
    );
    println!(
        "100 games, 400 persisted moves, 200 spectator streams; report: {}",
        output.display()
    );
    server.abort();

    Ok(())
}

#[tokio::test]
async fn metrics_endpoint_exposes_worker_counters() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let (status, body) = app.get_text("/metrics").await?;
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

    Ok(())
}

// Use a multithread runtime for blocking Solana health checks.
#[tokio::test(flavor = "multi_thread")]
async fn health_detailed_reports_real_memory_and_disk_state() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let (status, body) = app.get("/health/detailed").await?;
    // Only "critical" returns 503; "degraded" (e.g. this test's unreachable RPC)
    // stays 200. Whether the host is critical depends on the machine running the
    // test (a nearly full disk is), so check the code matches the reported status.
    let critical = body["status"] == "critical";
    let expected = if critical {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    assert_eq!(status, expected, "{body}");

    let checks = body["checks"].as_array().context("checks array")?;
    let find = |name: &str| {
        checks
            .iter()
            .find(|c| c["name"] == name)
            .with_context(|| format!("no {name} check in {checks:?}"))
    };

    // Both checks must report a real usage percentage (not the old placeholder
    // strings) and classify it: over 90% critical, over 80% warning, else ok.
    for name in ["memory", "disk_space"] {
        let check = find(name)?;
        let msg = check["message"].as_str().unwrap_or_default();
        let percent = usage_percent(msg)
            .with_context(|| format!("{name} check should report a usage percentage: {msg}"))?;
        let expected = if percent > 90.0 {
            "critical"
        } else if percent > 80.0 {
            "warning"
        } else {
            "ok"
        };
        assert_eq!(check["status"], expected, "{name} check: {check}");
    }

    Ok(())
}

/// The first `NN.N%` in a health-check message.
fn usage_percent(msg: &str) -> Option<f64> {
    let end = msg.find('%')?;
    let start = msg[..end]
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map_or(0, |i| i + 1);
    msg[start..end].parse().ok()
}

#[tokio::test]
async fn blur_telemetry_requires_a_wallet_identity() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    // Blur telemetry is anti-cheat evidence against a named player, so it
    // moved behind wallet auth; an anonymous report is rejected outright.
    let (status, _) = app
        .post_json(
            "/telemetry/blur",
            &json!({ "game_id": 999001, "move_number": 1, "color": "white", "blurred": true }),
        )
        .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn blur_telemetry_unknown_game_is_404() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let token = app
        .state
        .jwt
        .issue(&Keypair::new().pubkey().to_string())
        .context("issue jwt")?;
    let (status, _) = app
        .send_auth(
            "POST",
            "/telemetry/blur",
            Some(&token),
            Some(
                &json!({ "game_id": 999001, "move_number": 1, "color": "white", "blurred": true }),
            ),
        )
        .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn blur_telemetry_enforces_ply_parity() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let game_id: u64 = 424242;
    // A session must exist for the game before telemetry is accepted.
    app.state
        .store
        .create(game_id, solana_sdk::pubkey::Pubkey::new_unique())
        .await
        .context("create session")?;
    let token = app
        .state
        .jwt
        .issue(&Keypair::new().pubkey().to_string())
        .context("issue jwt")?;

    // Ply 1 claimed as black — parity violation rejected.
    let (bad_status, _) = app
        .send_auth(
            "POST",
            "/telemetry/blur",
            Some(&token),
            Some(
                &json!({ "game_id": game_id, "move_number": 1, "color": "black", "blurred": true }),
            ),
        )
        .await?;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);

    // Ply 0 is invalid.
    let (zero_status, _) = app
        .send_auth(
            "POST",
            "/telemetry/blur",
            Some(&token),
            Some(&json!({ "game_id": game_id, "move_number": 0, "color": "white", "blurred": false })),
        )
        .await?;
    assert_eq!(zero_status, StatusCode::BAD_REQUEST);

    // Correct parity is insufficient: the reporter must be a verified participant.
    let (ok_parity, _) = app
        .send_auth(
            "POST",
            "/telemetry/blur",
            Some(&token),
            Some(&json!({ "game_id": game_id, "move_number": 1, "color": "white", "blurred": false, "think_ms": 3000 })),
        )
        .await?;
    assert_ne!(ok_parity, StatusCode::BAD_REQUEST);
    assert_ne!(ok_parity, StatusCode::NO_CONTENT);

    Ok(())
}

#[tokio::test]
async fn broadcast_delay_gates_public_move_feed() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let repo = app.repo();
    let game = "770077";

    // Live game (delay 0): create the row, add two (now-stamped) moves.
    repo.set_broadcast_delay(game, 0)
        .await
        .context("test setup failed")?;
    repo.add_move_simple(game, 1, "e2e4", None, Some("fen1"), "white")
        .await
        .context("test setup failed")?;
    repo.add_move_simple(game, 2, "e7e5", None, Some("fen2"), "black")
        .await
        .context("test setup failed")?;

    let (status, body) = app.get(&format!("/games/moves/{game}")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["moves"].as_array().context("test setup failed")?.len(),
        2,
        "live feed shows all moves"
    );

    // Apply a 1-hour delay: the just-recorded moves are inside the window and
    // must disappear from the public feed.
    let (status, body) = app
        .get(&format!("/games/moves/{game}?after_move=1"))
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reset"], false);
    assert_eq!(
        body["moves"],
        json!([{ "move_number": 2, "move_uci": "e7e5" }])
    );
    let (status, body) = app
        .get(&format!("/games/moves/{game}?after_move=2"))
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["moves"], json!([]));
    repo.set_broadcast_delay(game, 3600)
        .await
        .context("test setup failed")?;
    let (status, body) = app.get(&format!("/games/moves/{game}")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["moves"].as_array().context("test setup failed")?.len(),
        0,
        "delayed feed withholds recent moves"
    );
    let (status, body) = app
        .get(&format!("/games/moves/{game}?after_move=2"))
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reset"], true);
    assert_eq!(body["moves"], json!([]));

    // The delay is reported for the spectator client's pre-subscribe check.
    let (status, body) = app.get(&format!("/games/{game}/broadcast-delay")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["delay_secs"].as_i64().context("test setup failed")?,
        3600
    );

    Ok(())
}

#[tokio::test]
async fn presence_heartbeat_returns_count_and_targeted_lookup_obeys_expiry() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let (status, body) = app
        .admin_request(
            "PUT",
            "/presence",
            Some(&json!({
                "node_id": "fresh", "pubkey": null, "display_name": "Fresh",
                "status": "online", "game_id": null, "updated_at": chrono::Utc::now(),
            })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["online_count"], 1);
    let (status, body) = app.get("/presence/fresh").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["node_id"], "fresh");
    let (status, body) = app.get("/presence/missing").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_null());
    let (status, body) = app
        .admin_request(
            "PUT",
            "/presence",
            Some(&json!({
                "node_id": "stale", "pubkey": null, "display_name": "Stale",
                "status": "online", "game_id": null,
                "updated_at": chrono::Utc::now() - chrono::Duration::seconds(60),
            })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["online_count"], 1);
    let (_, body) = app.get("/presence/stale").await?;
    assert!(body.is_null());
    let (_, body) = app.get("/presence").await?;
    assert_eq!(
        body.as_array().context("test setup failed")?.len(),
        1,
        "legacy roster remains available"
    );

    Ok(())
}

#[tokio::test]
async fn completed_game_surfaces_in_history() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let repo = app.repo();
    let white = "WALLET_WHITE_E2E";
    let black = "WALLET_BLACK_E2E";

    repo.complete_game(backend::db::repository::CompletedGame {
        game_id: "histgame1",
        player_white: Some(white),
        player_black: Some(black),
        white_username: Some("alice"),
        black_username: Some("bob"),
        winner: Some(white),
        final_fen: None,
        finalize_sig: "test-sig",
        stake_amount: 0.0,
    })
    .await
    .context("test setup failed")?;

    let (status, body) = app.get(&format!("/games/history/{white}")).await?;
    assert_eq!(status, StatusCode::OK);
    let games = body["games"].as_array().context("games array")?;
    assert!(
        games.iter().any(|g| g["id"] == "histgame1"),
        "completed game should appear in player history: {body}"
    );

    Ok(())
}

#[tokio::test]
async fn dispute_notify_then_status() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let game_id = 5150;
    let token = app.state.jwt.issue("WALLET_CHALLENGER")?;
    let notice = json!({
        "game_id": game_id,
        "challenger_wallet": "WALLET_CHALLENGER",
        "reason": "suspected engine use",
        "tx_signature": "sig-abc"
    });

    // Filing requires the challenger's own token.
    let (status, _) = app
        .send_auth("POST", "/dispute/notify", None, Some(&notice))
        .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let other = app.state.jwt.issue("WALLET_OTHER")?;
    let (status, _) = app
        .send_auth("POST", "/dispute/notify", Some(&other), Some(&notice))
        .await?;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = app
        .send_auth("POST", "/dispute/notify", Some(&token), Some(&notice))
        .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["case_id"], json!(format!("DISP-{game_id}")));

    // The dispute is now queryable.
    let (status, body) = app.get(&format!("/dispute/{game_id}")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["game_id"].as_i64().context("test setup failed")?,
        game_id
    );

    // Unknown dispute → 404.
    let (status, _) = app.get("/dispute/999999").await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

use solana_sdk::signature::{Keypair, Signer};

fn now_secs() -> anyhow::Result<u64> {
    Ok({
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("test setup failed")?
            .as_secs()
    })
}

impl TestApp {
    async fn send_auth(
        &self,
        method: &str,
        uri: &str,
        bearer: Option<&str>,
        body: Option<&Value>,
    ) -> anyhow::Result<(StatusCode, Value)> {
        Ok({
            let mut b = Request::builder().uri(uri).method(method);
            if let Some(t) = bearer {
                b = b.header("authorization", format!("Bearer {t}"));
            }
            let req = match body {
                Some(body) => b
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(body).context("test setup failed")?,
                    ))
                    .context("test setup failed")?,
                None => b.body(Body::empty()).context("test setup failed")?,
            };
            self.send(req).await?
        })
    }
}

#[tokio::test]
async fn auth_issue_endpoint_is_removed() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let (status, _) = app
        .post_json("/auth/issue", &json!({ "wallet_pubkey": "anything" }))
        .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn siws_login_then_logout_revokes_token() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();

    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await?;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().context("nonce")?.to_string();

    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().context("token")?.to_string();

    // A JWT-protected, chain-free route works with the fresh token.
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "e2eplayer" })),
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "authed route should accept fresh token"
    );

    // Cross a one-second boundary so the logout cut-off is strictly after `iat`.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let (status, _) = app
        .send_auth("POST", "/api/auth/logout", Some(&token), None)
        .await?;
    assert_eq!(status, StatusCode::OK, "logout should succeed");

    // The same token is now rejected.
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "e2eplayer2" })),
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revoked token must be rejected"
    );

    Ok(())
}

#[tokio::test]
async fn login_rejects_stale_timestamp() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let ts = now_secs()? - 4000; // well outside the 300s freshness window
    let sig = kp
        .sign_message(format!("xfchess:login:{ts}").as_bytes())
        .to_string();

    let (status, body) = app
        .post_json(
            "/api/auth/login",
            &json!({ "wallet": wallet, "signature": sig, "timestamp": ts }),
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "stale signature must be rejected: {body}"
    );

    Ok(())
}

#[tokio::test]
async fn link_wallet_rejects_repointing_an_already_linked_account() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let email = "linktest@example.com";
    let password = "correct horse battery staple";

    let (status, body) = app
        .post_json(
            "/api/auth/register-email",
            &json!({ "email": email, "password": password, "username": "linktester" }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "register-email: {body}");

    let first_wallet = Keypair::new();
    let first_wallet_pk = first_wallet.pubkey().to_string();
    let ts = now_secs()?;
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
        .await?;
    assert_eq!(status, StatusCode::OK, "first link should succeed: {body}");

    // Re-linking the SAME wallet again (e.g. a retried request) must still work.
    let ts2 = now_secs()?;
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
        .await?;
    assert_eq!(status, StatusCode::OK, "re-linking the same wallet: {body}");

    // Linking a DIFFERENT wallet must now be rejected.
    let second_wallet = Keypair::new();
    let second_wallet_pk = second_wallet.pubkey().to_string();
    let ts3 = now_secs()?;
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
        .await?;
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
                    .sign_message(format!("xfchess:login:{}", now_secs()?).as_bytes())
                    .to_string(),
                "timestamp": now_secs()?,
            }),
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "the original linked wallet must still be able to log in: {body}"
    );

    Ok(())
}

#[tokio::test]
async fn lichess_init_requires_auth_and_rejects_wallet_mismatch() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let stranger_wallet = Keypair::new().pubkey().to_string();

    // No Authorization header at all.
    let (status, _) = app
        .get(&format!("/api/auth/lichess/init?wallet_pubkey={wallet}"))
        .await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unauthenticated lichess/init must be rejected"
    );

    // Log in as `wallet`.
    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await?;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().context("nonce")?.to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().context("token")?.to_string();

    // A valid JWT for `wallet` trying to init a Lichess link for a
    // DIFFERENT wallet must be rejected.
    let (status, _) = app
        .send_auth(
            "GET",
            &format!("/api/auth/lichess/init?wallet_pubkey={stranger_wallet}"),
            Some(&token),
            None,
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a valid JWT for one wallet must not init a Lichess link for a different wallet"
    );

    // The owned request passes auth and then fails on missing OAuth config;
    // this assertion covers the guard, not the external flow.
    let (status, _) = app
        .send_auth(
            "GET",
            &format!("/api/auth/lichess/init?wallet_pubkey={wallet}"),
            Some(&token),
            None,
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "own-wallet request should clear the auth gate and fail only on missing Lichess config"
    );

    Ok(())
}

#[tokio::test]
async fn kyc_submit_requires_auth_and_rejects_wallet_mismatch() -> anyhow::Result<()> {
    let app = spawn_app().await?;
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
    let (status, _) = app.post_json("/api/kyc/submit", &kyc_body(&wallet)).await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unauthenticated KYC submission must be rejected"
    );

    // Log in as `wallet`, then try to submit KYC for a DIFFERENT wallet.
    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await?;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().context("nonce")?.to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().context("token")?.to_string();

    let (status, _) = app
        .send_auth(
            "POST",
            "/api/kyc/submit",
            Some(&token),
            Some(&kyc_body(&stranger_wallet)),
        )
        .await?;
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
        .await?;
    assert_eq!(status, StatusCode::OK, "own-wallet KYC submission: {body}");

    Ok(())
}

// Use a multi-threaded runtime for the blocking Solana client's block_in_place path.
#[tokio::test(flavor = "multi_thread")]
async fn offchain_username_does_not_imply_onchain_profile() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();

    let (status, body) = app
        .post_json("/api/auth/siws-challenge", &json!({ "wallet": wallet }))
        .await?;
    assert_eq!(status, StatusCode::OK, "challenge: {body}");
    let nonce = body["nonce"].as_str().context("nonce")?.to_string();
    let sig = kp
        .sign_message(format!("xfchess:siws:{nonce}").as_bytes())
        .to_string();
    let (status, body) = app
        .post_json(
            "/api/auth/siws-verify",
            &json!({ "wallet": wallet, "signature": sig, "nonce": nonce }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "verify: {body}");
    let token = body["token"].as_str().context("token")?.to_string();

    // Set an off-chain handle exactly as the wallet-ui's ProfileStep does on
    // first login (App.tsx's non-on-chain branch).
    let (status, _) = app
        .send_auth(
            "PATCH",
            "/api/auth/username",
            Some(&token),
            Some(&json!({ "username": "AlreadyRegistered" })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "username PATCH should succeed");

    // Verify off-chain username and on-chain profile existence remain independent
    // when the test RPC is unreachable.
    let (status, body) = app
        .send_auth("GET", "/api/auth/me", Some(&token), None)
        .await?;
    assert_eq!(status, StatusCode::OK, "me: {body}");
    assert_eq!(body["username"], "AlreadyRegistered");
    assert_eq!(
        body["has_onchain_profile"], false,
        "an off-chain username must never be mistaken for an on-chain profile"
    );

    // Missing on-chain profile and username must require the client’s handle-setup flow.
    let (status, body) = app
        .send_auth("POST", "/api/auth/sync-profile", Some(&token), None)
        .await?;
    assert_eq!(status, StatusCode::OK, "sync-profile: {body}");
    assert_eq!(body["has_profile"], false);
    assert_eq!(body["username_set"], false);

    Ok(())
}

static RELAY_TEST_LOCK: Mutex<()> = Mutex::const_new(());

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

#[tokio::test]
async fn dual_accept_auth_guards_signing_endpoints() -> anyhow::Result<()> {
    let _guard = RELAY_TEST_LOCK.lock().await;
    let _relay_secret = RelaySharedSecretGuard::set("e2e-relay-secret");
    let app = spawn_app().await?;

    let move_body = json!({ "game_id": 1, "move_uci": "e2e4", "next_fen": "x", "nonce": 1 });

    // (a) No auth at all → rejected by the guard.
    let (status, _) = app.post_json("/move/record", &move_body).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no auth must 401");

    // Relay-secret authentication carries no wallet identity; moves require a verified wallet.
    let req = Request::builder()
        .uri("/move/record")
        .method("POST")
        .header("content-type", "application/json")
        .header("X-Relay-Secret", "e2e-relay-secret")
        .body(Body::from(
            serde_json::to_vec(&move_body).context("test setup failed")?,
        ))
        .context("test setup failed")?;
    let (status, _) = app.send(req).await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "relay secret without a wallet identity must not record a move"
    );

    // (c) Per-user JWT (no relay header) → accepted.
    let kp = Keypair::new();
    let wallet = kp.pubkey().to_string();
    let token = app.state.jwt.issue(&wallet).context("issue jwt")?;
    let (status, _) = app
        .send_auth("POST", "/move/record", Some(&token), Some(&move_body))
        .await?;
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
        .await?;
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
        .await?;
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

    // Unset relay secret and absent JWT must fail closed.
    std::env::remove_var("RELAY_SHARED_SECRET");
    let (status, _) = app.post_json("/move/record", &move_body).await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "fail-closed: unset secret + no JWT must be rejected by the auth guard"
    );

    // A wallet JWT must not submit moves for a game it does not participate in.
    // A participant may still relay the opponent's move; this test covers rejection.
    let _relay_secret_g = RelaySharedSecretGuard::set("e2e-relay-secret");
    let mismatched_move_body = json!({
        "game_id": 1,
        "move_uci": "e2e4",
        "next_fen": "x",
        "nonce": 1,
        "mover_wallet": other,
    });
    let (status, _) = app
        .send_auth(
            "POST",
            "/move/record",
            Some(&token),
            Some(&mismatched_move_body),
        )
        .await?;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "JWT for a wallet with no on-chain relationship to this game must 403"
    );

    Ok(())
}

#[tokio::test]
async fn admin_route_requires_api_key() -> anyhow::Result<()> {
    let app = spawn_app().await?;
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
        .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn treasury_refund_never_signs_in_process() -> anyhow::Result<()> {
    let app = spawn_app().await?;
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
            .context("test setup failed")?,
        ))
        .context("test setup failed")?;
    let (status, body) = app.send(req).await?;

    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "awaiting_manual_execution");
    assert!(
        body.get("signature").is_none(),
        "no signature should ever come from this process: {body}"
    );
    // Structured binary + args (not a shell string), and the RPC URL — which
    // carries the provider token — is never echoed back unredacted.
    let handoff = &body["run_on_isolated_host"];
    assert_eq!(handoff["binary"], "treasury_signer", "{body}");
    let args = handoff["args"].as_array().context("args array")?;
    assert_eq!(args[0], wallet.as_str());
    assert_eq!(args[1], "1000000");
    assert!(handoff.get("command").is_none(), "no shell line: {body}");

    Ok(())
}

#[tokio::test]
async fn tournament_template_round_trip_persists_and_is_audited() -> anyhow::Result<()> {
    let app = spawn_app().await?;

    let (status, body) = app
        .admin_request(
            "POST",
            "/admin/tournament-templates",
            Some(&json!({ "name": "weekly-blitz", "data": { "max_players": 16, "format": "Swiss" } })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = app
        .admin_request("GET", "/admin/tournament-templates", None)
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let templates = body["templates"].as_array().context("templates array")?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0]["name"], "weekly-blitz");
    assert_eq!(templates[0]["data"]["max_players"], 16);

    // Audit persistence is awaited, so the save response implies the entry is visible.
    let (status, body) = app
        .admin_request("GET", "/admin/audit-log?limit=50", None)
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let entries = body["entries"].as_array().context("entries array")?;
    assert!(
        entries.iter().any(|e| e["action"] == "save_template"
            && e["target"] == "weekly-blitz"
            && e["actor"] == "dev-default"),
        "expected a persisted save_template audit entry: {entries:?}"
    );

    // Generic audit middleware must capture mutations without handler-specific
    // audit calls, including rejected requests.
    let (status, _) = app
        .admin_request(
            "POST",
            "/admin/tournament/999999/set-round-deadline",
            Some(&json!({ "deadline_at": 123 })),
        )
        .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = app
        .admin_request("GET", "/admin/audit-log?limit=50", None)
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let entries = body["entries"].as_array().context("entries array")?;
    assert!(
        entries.iter().any(|e| e["action"]
            .as_str()
            .is_some_and(|a| a.contains("set-round-deadline"))),
        "generic middleware should have logged the unhandled mutation: {entries:?}"
    );

    let (status, body) = app
        .admin_request("DELETE", "/admin/tournament-templates/weekly-blitz", None)
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = app
        .admin_request("GET", "/admin/tournament-templates", None)
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["templates"]
            .as_array()
            .context("test setup failed")?
            .len(),
        0
    );

    // Deleting an unknown template 404s rather than silently succeeding.
    let (status, _) = app
        .admin_request("DELETE", "/admin/tournament-templates/does-not-exist", None)
        .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn private_tournament_rejects_bad_password() -> anyhow::Result<()> {
    use backend::signing::storage::tournament::TournamentRecord;

    let app = spawn_app().await?;

    // Seed a private tournament directly — create_tournament itself submits
    // on-chain txs, which this in-process test deliberately never does.
    let mut record = TournamentRecord::new(4242, "private", 0);
    record.max_players = 4;
    record.password_hash = {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };
        let salt = SaltString::generate(&mut OsRng);
        Some(
            Argon2::default()
                .hash_password(b"correct-horse", &salt)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("test setup failed")?
                .to_string(),
        )
    };
    app.state.tournament_store.create(record).await;

    let stored = app
        .state
        .tournament_store
        .get(4242)
        .await
        .context("test setup failed")?;
    assert!(
        stored.password_hash.is_some(),
        "tournament should be private"
    );

    // Wrong password → rejected. A 403 here (rather than 401/422) means the
    // password gate fired, not the auth layer or signature verification.
    let wallet = Keypair::new().pubkey().to_string();
    let (status, _) = app
        .post_json(
            "/api/tournament/4242/confirm-join",
            &json!({
                "player": wallet,
                "elo": 1200,
                "signature": "not-a-real-signature",
                "password": "wrong"
            }),
        )
        .await?;
    assert_ne!(
        status,
        StatusCode::OK,
        "a wrong password must never register a player"
    );

    // Whatever the rejection reason, the roster must be untouched.
    let after = app
        .state
        .tournament_store
        .get(4242)
        .await
        .context("test setup failed")?;
    assert!(
        after.players.is_empty(),
        "no player should have been added: {:?}",
        after.players
    );

    Ok(())
}

// Simulate restart with a new AppState over the same SQLite databases.

fn relay_sign(kp: &Keypair, game_id: &str, message: &str) -> Vec<u8> {
    let signable = format!("{}:{}:{}", game_id, kp.pubkey(), message);
    kp.sign_message(signable.as_bytes()).as_ref().to_vec()
}

fn announce_body(game_id: &str, host: &str) -> Value {
    json!({
        "game_id": game_id, "host_node_id": host, "display_name": "host",
        "stake_amount": 0.0, "game_type": "P2P", "base_time_seconds": 300,
        "increment_seconds": 0, "username": null, "elo": null,
        "region": null, "password": null
    })
}

/// announce → join → accept, which also registers the immutable casual
/// participant pair the game log checks writers against.
async fn start_casual_game(
    app: &TestApp,
    game_id: &str,
    host: &str,
    joiner: &str,
) -> anyhow::Result<()> {
    let (_, b) = app
        .post_json("/p2p/announce", &announce_body(game_id, host))
        .await?;
    assert_eq!(b["success"], true);
    let (_, b) = app
        .post_json(
            "/p2p/join",
            &json!({ "game_id": game_id, "joiner_node_id": joiner, "password": null }),
        )
        .await?;
    assert_eq!(b["success"], true);
    let (_, b) = app
        .post_json(
            "/p2p/accept",
            &json!({ "game_id": game_id, "host_node_id": host }),
        )
        .await?;
    assert_eq!(b["success"], true);

    Ok(())
}

#[tokio::test]
async fn drill_lobby_and_join_ack_survive_backend_restart() -> anyhow::Result<()> {
    let session_url = unique_db_url("drill_relay_s");
    let vault_url = unique_db_url("drill_relay_v");
    let app = spawn_app_on(&session_url, &vault_url).await?;
    let host = Keypair::new();
    let joiner = Keypair::new();
    let game_id = "drill-relay-1";
    let announce = announce_body(game_id, &host.pubkey().to_string());

    let (_, b) = app.post_json("/p2p/announce", &announce).await?;
    assert_eq!(b["success"], true);
    let (_, b) = app
        .post_json(
            "/p2p/join",
            &json!({ "game_id": game_id, "joiner_node_id": joiner.pubkey().to_string(), "password": null }),
        )
        .await?;
    assert_eq!(b["success"], true);
    let ack = "JOIN_ACK:host|white|1200";
    let (_, b) = app
        .post_json(
            "/p2p/message",
            &json!({
                "game_id": game_id, "from_node_id": host.pubkey().to_string(),
                "message": ack, "signature": relay_sign(&host, game_id, ack)
            }),
        )
        .await?;
    assert_eq!(b["success"], true);

    // Backend restarts before the joiner polled the JOIN_ACK.
    let app = spawn_app_on(&session_url, &vault_url).await?;

    let (_, b) = app
        .post_json(
            "/p2p/poll",
            &json!({ "game_id": game_id, "node_id": joiner.pubkey().to_string(), "since_index": 0 }),
        )
        .await?;
    assert_eq!(b["messages"], json!([ack]), "JOIN_ACK survives the restart");
    assert_eq!(b["next_index"], 1);

    // A cursor from before the restart that is past the end must not panic.
    let (s, b) = app
        .post_json(
            "/p2p/poll",
            &json!({ "game_id": game_id, "node_id": joiner.pubkey().to_string(), "since_index": 50 }),
        )
        .await?;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["next_index"], 1);

    // Heartbeat finds the room; a same-host re-announce keeps the joiner;
    // a different node cannot take the room over.
    let (_, b) = app
        .post_json(
            "/p2p/heartbeat",
            &json!({ "game_id": game_id, "host_node_id": host.pubkey().to_string() }),
        )
        .await?;
    assert_eq!(b["success"], true);
    let (_, b) = app.post_json("/p2p/announce", &announce).await?;
    assert_eq!(b["success"], true);
    let (_, listing) = app.get("/p2p/games").await?;
    let room = listing
        .as_array()
        .context("test setup failed")?
        .iter()
        .find(|g| g["game_id"] == game_id)
        .cloned()
        .context("room listed after restart")?;
    assert_eq!(room["players_joined"], 2, "re-announce kept the joiner");
    let hijack = announce_body(game_id, &Keypair::new().pubkey().to_string());
    let (_, b) = app.post_json("/p2p/announce", &hijack).await?;
    assert_eq!(
        b["success"], false,
        "another node cannot overwrite the room"
    );

    // The handshake completes after the restart.
    let (_, b) = app
        .post_json(
            "/p2p/accept",
            &json!({ "game_id": game_id, "host_node_id": host.pubkey().to_string() }),
        )
        .await?;
    assert_eq!(b["success"], true);

    Ok(())
}

fn move_event(player: &str, uci: &str, fen: &str, n: u32, parent: &str) -> (Value, String) {
    let version = braid_chess::version_hash(fen, n);
    let body = json!({
        "player_pubkey": player,
        "session_token": "",
        "message": {
            "type": "move", "from": &uci[0..2], "to": &uci[2..4], "promotion": null,
            "uci": uci, "fen_after": fen, "move_number": n, "player": player
        },
        "content_version": version,
        "content_parent": parent,
    });
    (body, version)
}

async fn put_move(
    app: &TestApp,
    game_id: &str,
    body: &Value,
    bearer: Option<&str>,
) -> anyhow::Result<StatusCode> {
    Ok({
        app.send_auth("PUT", &format!("/game/{game_id}/moves"), bearer, Some(body))
            .await?
            .0
    })
}

async fn move_log(app: &TestApp, game_id: &str) -> anyhow::Result<Vec<Value>> {
    Ok({
        let (s, v) = app.get(&format!("/game/{game_id}/moves")).await?;
        assert_eq!(s, StatusCode::OK);
        v.as_array().cloned().unwrap_or_default()
    })
}

const FEN_E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
const FEN_E4E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";
const FEN_NF3: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";

#[tokio::test]
async fn drill_casual_game_survives_restart_without_duplicate_or_foreign_moves(
) -> anyhow::Result<()> {
    let session_url = unique_db_url("drill_log_s");
    let vault_url = unique_db_url("drill_log_v");
    let app = spawn_app_on(&session_url, &vault_url).await?;
    let host = Keypair::new().pubkey().to_string();
    let joiner = Keypair::new().pubkey().to_string();
    let game_id = "drill-casual-1";
    start_casual_game(&app, game_id, &host, &joiner).await?;

    let (m1, v1) = move_event(&host, "e2e4", FEN_E4, 1, "0");
    let (m2, v2) = move_event(&joiner, "e7e5", FEN_E4E5, 1, &v1);
    assert_eq!(put_move(&app, game_id, &m1, None).await?, StatusCode::OK);
    assert_eq!(put_move(&app, game_id, &m2, None).await?, StatusCode::OK);

    // Backend restarts mid-game.
    let app = spawn_app_on(&session_url, &vault_url).await?;

    assert_eq!(
        move_log(&app, game_id).await?.len(),
        2,
        "log survives restart"
    );
    // The joiner never saw its ack and retries the same write: idempotent.
    assert_eq!(put_move(&app, game_id, &m2, None).await?, StatusCode::OK);
    assert_eq!(
        move_log(&app, game_id).await?.len(),
        2,
        "retry is not a second move"
    );
    // A third party naming itself cannot write into the game after restart.
    let stranger = Keypair::new().pubkey().to_string();
    let (foreign, _) = move_event(&stranger, "g1f3", FEN_NF3, 2, &v2);
    assert_eq!(
        put_move(&app, game_id, &foreign, None).await?,
        StatusCode::FORBIDDEN
    );
    // Play continues on the persisted head.
    let (m3, _) = move_event(&host, "g1f3", FEN_NF3, 2, &v2);
    assert_eq!(put_move(&app, game_id, &m3, None).await?, StatusCode::OK);
    let log = move_log(&app, game_id).await?;
    assert_eq!(log.len(), 3);
    assert_eq!(log[2]["fen_after"], FEN_NF3);

    Ok(())
}

#[tokio::test]
async fn drill_second_device_takes_the_seat_and_first_becomes_view_only() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let host = Keypair::new().pubkey().to_string();
    let joiner = Keypair::new().pubkey().to_string();
    let game_id = "drill-seat-1";
    start_casual_game(&app, game_id, &host, &joiner).await?;
    let host_jwt = app.state.jwt.issue(&host).context("jwt")?;

    // Device A holds the host seat. (The claim route checks on-chain
    // participation, unavailable here, so the lease is taken via the store.)
    app.state
        .seat_leases
        .claim(game_id, &host, "device-aaaa-0001", None, 1)
        .await
        .context("test setup failed")?;
    let (mut m1, v1) = move_event(&host, "e2e4", FEN_E4, 1, "0");
    m1["device_id"] = json!("device-aaaa-0001");
    assert_eq!(
        put_move(&app, game_id, &m1, Some(&host_jwt)).await?,
        StatusCode::OK
    );

    // The opponent never claimed a seat: legacy behaviour.
    let (opp, v2) = move_event(&joiner, "e7e5", FEN_E4E5, 1, &v1);
    assert_eq!(put_move(&app, game_id, &opp, None).await?, StatusCode::OK);

    // Once claimed, the seat also needs the wallet token, not just its name.
    let (mut no_token, _) = move_event(&host, "g1f3", FEN_NF3, 2, &v2);
    no_token["device_id"] = json!("device-aaaa-0001");
    assert_eq!(
        put_move(&app, game_id, &no_token, None).await?,
        StatusCode::UNAUTHORIZED
    );

    // Device B (same wallet) takes over; A is now view-only.
    let lease = app
        .state
        .seat_leases
        .claim(game_id, &host, "device-bbbb-0002", None, 2)
        .await
        .context("test setup failed")?;
    assert_eq!(lease.epoch, 2);
    let mut from_a = no_token.clone();
    from_a["device_id"] = json!("device-aaaa-0001");
    assert_eq!(
        put_move(&app, game_id, &from_a, Some(&host_jwt)).await?,
        StatusCode::CONFLICT,
        "superseded device cannot move"
    );
    let mut from_b = from_a.clone();
    from_b["device_id"] = json!("device-bbbb-0002");
    assert_eq!(
        put_move(&app, game_id, &from_b, Some(&host_jwt)).await?,
        StatusCode::OK
    );
    assert_eq!(move_log(&app, game_id).await?.len(), 3);

    // Seat routes: readable by the wallet; claiming needs auth + participation.
    let (s, v) = app
        .send_auth("GET", "/game/77001/seat", Some(&host_jwt), None)
        .await?;
    assert_eq!((s, v), (StatusCode::OK, Value::Null));
    let claim = json!({ "device_id": "device-cccc-0003" });
    let (s, _) = app
        .send_auth(
            "POST",
            "/game/77001/seat/claim",
            Some(&host_jwt),
            Some(&claim),
        )
        .await?;
    assert_eq!(
        s,
        StatusCode::FORBIDDEN,
        "unverifiable participant cannot claim"
    );
    let (s, _) = app
        .send_auth("POST", "/game/77001/seat/claim", None, Some(&claim))
        .await?;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // The move-signing key registered with a claim must be a real public key.
    let bad_signer = json!({ "device_id": "device-cccc-0003", "move_signer": "not-a-key" });
    let (s, _) = app
        .send_auth(
            "POST",
            "/game/77001/seat/claim",
            Some(&host_jwt),
            Some(&bad_signer),
        )
        .await?;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn casual_player_can_write_more_than_one_move_in_the_same_backend_process(
) -> anyhow::Result<()> {
    // Casual Iroh node IDs parse as Pubkeys but must not require on-chain session keys.
    let app = spawn_app().await?;
    let host = Keypair::new().pubkey().to_string();
    let joiner = Keypair::new().pubkey().to_string();
    let game_id = "casual-second-write";
    start_casual_game(&app, game_id, &host, &joiner).await?;

    let (m1, v1) = move_event(&host, "e2e4", FEN_E4, 1, "0");
    let (m2, v2) = move_event(&joiner, "e7e5", FEN_E4E5, 1, &v1);
    let (m3, v3) = move_event(&host, "g1f3", FEN_NF3, 2, &v2);
    let fen4 = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3";
    let (m4, _) = move_event(&joiner, "b8c6", fen4, 2, &v3);
    for m in [&m1, &m2, &m3, &m4] {
        assert_eq!(put_move(&app, game_id, m, None).await?, StatusCode::OK);
    }
    assert_eq!(move_log(&app, game_id).await?.len(), 4);

    Ok(())
}

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Adds a puzzle whose line is setup 1.e4, then player ...e5, reply 2.Nf3,
/// player ...Nc6.
async fn insert_puzzle(app: &TestApp, id: &str, themes: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO puzzles (id, fen, line, rating, themes) VALUES (?, ?, ?, 1500, ?)")
        .bind(id)
        .bind(START_FEN)
        .bind("e2e4 e7e5 g1f3 b8c6")
        .bind(themes)
        .execute(&app.state.store.pool())
        .await
        .context("test setup failed")?;

    Ok(())
}

#[tokio::test]
async fn puzzle_next_filters_by_theme_and_validates_input() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    insert_puzzle(&app, "pFork", "fork middlegame").await?;
    insert_puzzle(&app, "pPin", "pin short").await?;

    for _ in 0..5 {
        let (status, body) = app
            .get("/puzzle/next?wallet=themeW&theme=fork&difficulty=harder")
            .await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["id"], "pFork", "{body}");
        assert_eq!(body["themes"], json!(["fork", "middlegame"]));
        assert_eq!(body["source"], "local");
        assert_eq!(body["color"], "black");
    }

    let (status, _) = app.get("/puzzle/next?wallet=themeW&theme=nope").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .get("/puzzle/next?wallet=themeW&difficulty=insane")
        .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, themes) = app.get("/puzzle/themes").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(themes
        .as_array()
        .context("test setup failed")?
        .iter()
        .any(|t| t["key"] == "fork" && t["label"] == "Fork"));

    Ok(())
}

#[tokio::test]
async fn puzzle_from_lichess_requires_wallet_auth() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let (status, _) = app
        .get("/puzzle/next?wallet=lichessW&source=lichess")
        .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    Ok(())
}

/// Plays the test line to the end and returns the final move's response.
async fn solve(app: &TestApp, wallet: &str, hint: bool) -> anyhow::Result<Value> {
    Ok({
        let (_, puzzle) = app.get(&format!("/puzzle/next?wallet={wallet}")).await?;
        let nonce = puzzle["nonce"].as_str().context("nonce")?.to_owned();
        if hint {
            let (status, h) = app
                .post_json("/puzzle/hint", &json!({ "nonce": nonce }))
                .await?;
            assert_eq!(status, StatusCode::OK, "{h}");
            assert_eq!(h, json!({ "from": "e7", "rated": false }));
        }
        let (_, first) = app
            .post_json("/puzzle/move", &json!({ "nonce": nonce, "uci": "e7e5" }))
            .await?;
        assert_eq!(
            first,
            json!({ "correct": true, "done": false, "reply": "g1f3" })
        );
        let (_, last) = app
            .post_json("/puzzle/move", &json!({ "nonce": nonce, "uci": "b8c6" }))
            .await?;
        assert_eq!(last["win"], true, "{last}");
        last
    })
}

#[tokio::test]
async fn puzzle_hint_makes_the_round_unrated() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    insert_puzzle(&app, "pLine", "opening").await?;

    let rated = solve(&app, "ratedW", false).await?;
    assert!(
        rated["rating_diff"].as_i64().context("test setup failed")? > 0,
        "{rated}"
    );

    let hinted = solve(&app, "hintW", true).await?;
    assert_eq!(hinted["rating_diff"], 0, "{hinted}");
    let (_, rating) = app.get("/puzzle/rating/hintW").await?;
    assert_eq!(rating["rating"], hinted["rating"]);

    let flags: Vec<(String, i64)> =
        sqlx::query_as("SELECT wallet, rated FROM puzzle_rounds ORDER BY wallet")
            .fetch_all(&app.state.store.pool())
            .await
            .context("test setup failed")?;
    assert_eq!(flags, [("hintW".into(), 0), ("ratedW".into(), 1)]);

    Ok(())
}

/// Settlement bookkeeping records what the chain settled — never a caller's
/// claimed winner — forwards tournament results and retires the session.
// Multithread: rating lookups use the blocking Solana client.
#[tokio::test(flavor = "multi_thread")]
async fn settlement_bookkeeping_uses_the_chain_result() -> anyhow::Result<()> {
    use backend::signing::solana::game_account::{GameAccount, RESULT_WINNER, STATUS_FINISHED};
    use backend::signing::storage::tournament::GameOutcome;
    use backend::signing::OrchestratorEvent;
    use solana_sdk::pubkey::Pubkey;

    let mut app = spawn_app().await?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    app.state.orchestrator_tx = Some(tx);

    let game_id = 4_242;
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    app.state
        .store
        .create(game_id, white)
        .await
        .context("test setup failed")?;
    let snap = GameAccount {
        game_id,
        white,
        black,
        status: STATUS_FINISHED,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: white,
        result_tag: RESULT_WINNER,
        winner: Some(black),
        move_count: 40,
        halfmove_clock: 0,
        turn: 41,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        country_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 0,
        is_delegated: false,
        tournament_id: Some(9),
        nonce: 40,
    };

    backend::tasks::settlement_worker::record_settled_game(
        &app.state,
        game_id,
        &snap,
        Some("settlement-sig"),
    )
    .await;

    let record = app
        .repo()
        .get_game(&game_id.to_string())
        .await
        .context("test setup failed")?
        .context("game recorded")?;
    assert_eq!(record.status, "completed");
    assert_eq!(record.winner.as_deref(), Some("black"));
    assert_eq!(record.finalize_sig.as_deref(), Some("settlement-sig"));

    match rx.try_recv().context("tournament result forwarded")? {
        OrchestratorEvent::GameEnded {
            tournament_id,
            game_id: ended,
            outcome,
            ..
        } => {
            assert_eq!((tournament_id, ended), (9, game_id));
            assert!(matches!(outcome, GameOutcome::BlackWin));
        }
        other => anyhow::bail!("unexpected orchestrator event: {other:?}"),
    }

    let active = app
        .state
        .store
        .try_list_active_game_ids()
        .await
        .context("test setup failed")?;
    assert!(
        !active.contains(&game_id),
        "the settled game's session is retired"
    );

    Ok(())
}

#[tokio::test]
async fn valid_bearer_is_rejected_when_revocation_storage_is_unavailable() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let token = app.state.jwt.issue("wallet")?;
    sqlx::query("DROP TABLE jwt_revocations")
        .execute(&app.state.store.pool())
        .await?;
    let request = Request::builder()
        .method("POST")
        .uri("/session/create")
        .header("Authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(
            &json!({"game_id":55,"wallet_pubkey":"wallet"}),
        )?))?;
    let (status, _) = app.send(request).await?;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    Ok(())
}
