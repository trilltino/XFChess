//! Route-authorisation matrix (test plan WP2).
//!
//! Every route the real router registers must be classified in `classify`. A route that is
//! missing fails `every_route_is_classified`, so a new endpoint cannot ship without someone
//! deciding who may call it. The classes that a generic probe can verify are then exercised
//! against the real router: admin routes reject missing, wrong and wallet-only credentials,
//! and wallet routes reject missing, malformed, wrongly signed and expired tokens.
//!
//! Node routes (lobby relay, social graph, client events) are keyed by a network node id, not a
//! wallet: they need an `X-Node-Auth` token signed by that node's key, and every node id named
//! in the request must be the one that signed. No route is left without a credential check.
#![cfg(feature = "coverage")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use jsonwebtoken::{encode, EncodingKey, Header};
use solana_sdk::signature::{Keypair, Signer};
use std::collections::BTreeSet;
use tower::ServiceExt;

const WALLET_A: &str = "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C";
const WALLET_B: &str = "DuvD57712cubVRwHbzNTmJoZzpEMMYDXTTcSGtKQ21Fv";
const JWT_SECRET: &str = "test-secret-not-for-production";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Anyone may call it: read-only data, health, or unsigned-transaction builders.
    Public,
    /// `X-API-Key` required (admin middleware).
    Admin,
    /// Wallet JWT enforced by middleware or extractor; anonymous callers get 401.
    Wallet,
    /// Credential verified inside the handler (JWT, wallet signature, nonce or OAuth proof).
    /// Needs a well-formed body to reach the check, so the generic probe cannot drive it.
    Handler,
    /// `X-Node-Auth` token signed by the node the request acts for (see `signing::node_auth`).
    Node,
}

use Class::{Admin, Handler, Node, Public, Wallet};

/// Paths that exist under both `/api/tournament` and `/tournament`.
fn both(prefixes: &[&str], suffixes: &[(&str, &str, Class)]) -> Vec<(String, String, Class)> {
    prefixes
        .iter()
        .flat_map(|prefix| {
            suffixes.iter().map(move |(method, suffix, class)| {
                (method.to_string(), format!("{prefix}{suffix}"), *class)
            })
        })
        .collect()
}

fn table() -> Vec<(String, String, Class)> {
    let single: &[(&str, &str, Class)] = &[
        // ── Public reads ────────────────────────────────────────────────────────────
        ("GET", "/health", Public),
        ("GET", "/health/detailed", Public),
        ("GET", "/readyz", Public),
        ("GET", "/metrics", Public),
        ("GET", "/stats", Public),
        ("GET", "/region", Public),
        ("GET", "/api/runtime-info", Public),
        ("GET", "/api/rates/all", Public),
        ("GET", "/api/rates/platform-fee", Public),
        ("GET", "/api/rates/sol-gbp", Public),
        ("GET", "/api/tournaments", Public),
        ("GET", "/tournaments", Public),
        ("GET", "/api/tournaments/my", Public),
        ("GET", "/tournaments/my", Public),
        ("GET", "/offline-tournaments", Public),
        ("GET", "/offline-tournaments/{id}", Public),
        ("GET", "/api/auth/check-username/{username}", Public),
        ("GET", "/api/auth/check-wallet/{wallet}", Public),
        ("GET", "/api/auth/lichess/callback", Public),
        ("GET", "/api/auth/lichess/init", Public),
        ("GET", "/api/external-elo/status/{pubkey}", Public),
        ("GET", "/api/user/status/{pubkey}", Public),
        ("GET", "/api/anticheat/stats/{pubkey}", Public),
        ("GET", "/api/anticheat/verdict/{game_id}", Public),
        ("GET", "/api/wallet/balance/{pubkey}", Public),
        ("GET", "/api/debug/tx/{signature}", Public),
        ("GET", "/api/global-session/{wallet}/verify", Public),
        ("GET", "/identity/status/{pubkey}", Public),
        ("GET", "/player/{pubkey}", Public),
        ("GET", "/games/history/username/{username}", Public),
        ("GET", "/games/history/{wallet}", Public),
        ("GET", "/games/active/{wallet}", Public),
        ("GET", "/games/moves/{game_id}", Public),
        ("GET", "/games/{game_id}/broadcast-delay", Public),
        ("GET", "/games/{game_id}/pgn", Public),
        ("GET", "/game/{game_id}/chat", Public),
        ("GET", "/game/{game_id}/moves", Public),
        ("GET", "/game/{game_id}/nodes", Public),
        ("GET", "/game/{game_id}/participants", Public),
        ("GET", "/game/{game_id}/er-endpoint", Public),
        ("GET", "/game/{game_id}/nonce", Public),
        ("GET", "/spectate/{game_id}/moves", Public),
        ("GET", "/dispute/{game_id}", Public),
        ("GET", "/session/status/{game_id}", Public),
        ("GET", "/matchmaking/status/{pubkey}", Public),
        ("GET", "/p2p/games", Public),
        ("GET", "/presence", Public),
        ("GET", "/presence/summary", Public),
        ("GET", "/presence/{node_id}", Public),
        ("GET", "/puzzle/daily", Public),
        ("GET", "/puzzle/next", Public),
        ("GET", "/puzzle/themes", Public),
        ("GET", "/puzzle/rating/{wallet}", Public),
        ("GET", "/ratings/history/{wallet}", Public),
        // Blinks: build unsigned transactions for the account named in the request.
        ("GET", "/api/actions/tournament/{id}", Public),
        ("GET", "/api/actions/tournament/{id}/check-balance", Public),
        ("GET", "/api/actions/tournament/{id}/claim-prize", Public),
        (
            "GET",
            "/api/actions/tournament/{id}/chain/onboarding",
            Public,
        ),
        (
            "GET",
            "/api/actions/tournament/{id}/chain/registration",
            Public,
        ),
        ("POST", "/api/actions/tournament/{id}/claim-prize", Public),
        ("POST", "/api/actions/tournament/{id}/register", Public),
        (
            "POST",
            "/api/actions/tournament/{id}/register/confirm",
            Public,
        ),
        ("POST", "/api/actions/tournament/{id}/validate", Public),
        ("POST", "/api/signup", Public),
        // Origin-gated RPC proxy; rejects callers without an allowed Origin.
        ("POST", "/api/rpc", Public),
        ("POST", "/api/rpc/mainnet", Public),
        // ── Wallet JWT enforced before the handler ──────────────────────────────────
        ("GET", "/api/auth/me", Wallet),
        ("POST", "/api/auth/logout", Wallet),
        ("POST", "/api/auth/sync-profile", Wallet),
        ("GET", "/api/money-actions/by-scope", Wallet),
        ("GET", "/api/money-actions/{id}", Wallet),
        ("POST", "/api/money-actions", Wallet),
        ("DELETE", "/api/global-session/{wallet}", Wallet),
        ("POST", "/api/global-session/register", Wallet),
        ("POST", "/api/global-session/track-game", Wallet),
        ("GET", "/game/{game_id}/seat", Wallet),
        ("POST", "/game/{game_id}/seat/claim", Wallet),
        ("POST", "/game/{game_id}/recover", Wallet),
        ("POST", "/game/delegate", Wallet),
        ("POST", "/game/finalize", Wallet),
        ("POST", "/game/undelegate", Wallet),
        ("POST", "/move/record", Wallet),
        ("POST", "/ratings/update", Wallet),
        ("POST", "/session/abandon/{game_id}", Wallet),
        ("POST", "/session/activate", Wallet),
        ("POST", "/session/create", Wallet),
        ("POST", "/telemetry/blur", Wallet),
        // ── Credential checked inside the handler ───────────────────────────────────
        ("POST", "/api/auth/siws-challenge", Handler),
        ("POST", "/api/auth/siws-verify", Handler),
        ("POST", "/api/auth/login", Handler),
        ("POST", "/api/auth/login-email", Handler),
        ("POST", "/api/auth/register", Handler),
        ("POST", "/api/auth/register-email", Handler),
        ("POST", "/api/auth/privy-login", Handler),
        ("POST", "/api/auth/link-wallet", Handler),
        ("POST", "/api/auth/delete", Handler),
        ("POST", "/api/auth/add-email", Handler),
        ("PATCH", "/api/auth/username", Handler),
        ("POST", "/api/auth/init-profile-tx", Handler),
        ("POST", "/api/auth/init-profile-sponsored-tx", Handler),
        ("POST", "/api/auth/lichess/exchange", Handler),
        ("POST", "/api/kyc/submit", Handler),
        ("POST", "/api/games/casual", Handler),
        ("POST", "/identity/register", Handler),
        ("POST", "/identity/delete", Handler),
        ("POST", "/matchmaking/join", Handler),
        ("POST", "/matchmaking/leave", Handler),
        ("POST", "/p2p/message", Handler),
        ("POST", "/puzzle/move", Handler),
        ("POST", "/puzzle/hint", Handler),
        ("GET", "/ws/auth", Handler),
        // ── No credential check: reviewed debt ──────────────────────────────────────
        // Signed-in users only; the token is checked inside the handler.
        ("POST", "/api/auth/broadcast-tx", Handler),
        ("POST", "/api/external-elo/link/start", Wallet),
        ("POST", "/api/external-elo/link/confirm", Wallet),
        ("POST", "/api/external-elo/sync", Wallet),
        ("POST", "/api/client-events", Node),
        ("POST", "/dispute/notify", Wallet),
        ("GET", "/friends", Node),
        ("DELETE", "/friends/{contact_id}", Node),
        ("POST", "/friends/invite", Node),
        ("GET", "/friends/requests", Node),
        ("POST", "/friends/requests", Node),
        ("PUT", "/friends/requests/{id}", Node),
        ("PUT", "/presence", Node),
        ("GET", "/social/poll", Node),
        // Relay lobby: /p2p/message carries its own signature.
        ("POST", "/p2p/announce", Node),
        ("POST", "/p2p/accept", Node),
        ("POST", "/p2p/heartbeat", Node),
        ("POST", "/p2p/join", Node),
        ("POST", "/p2p/leave", Node),
        ("POST", "/p2p/poll", Node),
        // The sender must prove the identity it writes as (wallet token or node token).
        ("PUT", "/game/{game_id}/chat", Handler),
        ("PUT", "/game/{game_id}/moves", Handler),
    ];
    let mut rows: Vec<_> = single
        .iter()
        .map(|(m, p, c)| (m.to_string(), p.to_string(), *c))
        .collect();
    let prefixes = ["/api/tournament", "/tournament"];
    rows.extend(both(
        &prefixes,
        &[
            ("GET", "/{id}", Public),
            ("GET", "/{id}/bootstrap-peers", Public),
            ("GET", "/{id}/bracket", Public),
            ("GET", "/{id}/current-round", Public),
            ("GET", "/{id}/games", Public),
            ("GET", "/{id}/my-match", Public),
            ("GET", "/{id}/my-status", Public),
            ("GET", "/{id}/pairings/{round}", Public),
            ("GET", "/{id}/registration-info", Public),
            ("GET", "/{id}/schedule-status", Public),
            ("GET", "/{id}/standings", Public),
            ("GET", "/{id}/build-register-tx", Wallet),
            ("POST", "/{id}/build-leave-tx", Wallet),
            ("POST", "/{id}/confirm-join", Wallet),
            ("POST", "/{id}/join", Wallet),
            ("POST", "/{id}/leave", Wallet),
            ("POST", "/{id}/register-node", Wallet),
            ("POST", "/{id}/session-create-game", Wallet),
            ("POST", "/{id}/session-join-game", Wallet),
            ("POST", "/{id}/subscribe-node", Wallet),
        ],
    ));
    rows
}

/// Anything under an admin prefix is `Admin`; everything else must be in `table`.
fn classify(method: &str, path: &str, table: &[(String, String, Class)]) -> Option<Class> {
    if path.starts_with("/admin/") || path.contains("/admin/") {
        return Some(Admin);
    }
    table
        .iter()
        .find(|(m, p, _)| m == method && p == path)
        .map(|(_, _, class)| *class)
}

/// Replaces `{param}` segments with a value of the right shape.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| match segment {
            "{wallet}" | "{pubkey}" | "{contact_id}" => WALLET_A,
            s if s.starts_with('{') && s.ends_with('}') => "1",
            s => s,
        })
        .collect::<Vec<_>>()
        .join("/")
}

async fn runtime_routes(app: &common::TestApp) -> anyhow::Result<BTreeSet<(String, String)>> {
    let router = app.router();
    let mut rows = backend::coverage::runtime_routes(&router).map_err(anyhow::Error::msg)?;
    // `/braid` is a separately inventoried child service with its own auth model.
    rows.retain(|(_, path)| !path.starts_with("/braid"));
    Ok(rows
        .into_iter()
        .filter(|(method, _)| method != "ANY")
        .collect())
}

fn routes_of(
    class: Class,
    routes: &BTreeSet<(String, String)>,
    table: &[(String, String, Class)],
) -> Vec<(String, String)> {
    routes
        .iter()
        .filter(|(m, p)| classify(m, p, table) == Some(class))
        .cloned()
        .collect()
}

async fn call(
    app: &common::TestApp,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
) -> anyhow::Result<StatusCode> {
    let mut builder = Request::builder()
        .method(method)
        .uri(concrete(path))
        .header("content-type", "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    let response = app
        .router()
        .oneshot(builder.body(Body::from("{}"))?)
        .await?;
    Ok(response.status())
}

fn bearer(token: &str) -> (&'static str, String) {
    ("authorization", format!("Bearer {token}"))
}

fn forged_token(secret: &str, wallet: &str, expires_in: i64) -> anyhow::Result<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({ "sub": wallet, "iat": now - 10, "exp": now + expires_in });
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

#[tokio::test]
async fn every_route_is_classified() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let table = table();
    let routes = runtime_routes(&app).await?;
    let unclassified: Vec<_> = routes
        .iter()
        .filter(|(m, p)| classify(m, p, &table).is_none())
        .collect();
    anyhow::ensure!(
        unclassified.is_empty(),
        "routes with no auth classification (add them to `table`): {unclassified:#?}"
    );
    let stale: Vec<_> = table
        .iter()
        .filter(|(m, p, _)| !routes.contains(&(m.clone(), p.clone())))
        .map(|(m, p, _)| format!("{m} {p}"))
        .collect();
    anyhow::ensure!(
        stale.is_empty(),
        "table lists routes the router no longer registers: {stale:#?}"
    );
    Ok(())
}

#[tokio::test]
async fn admin_routes_require_the_admin_key() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let table = table();
    let routes = runtime_routes(&app).await?;
    let admin = routes_of(Admin, &routes, &table);
    anyhow::ensure!(
        admin.len() > 60,
        "admin surface unexpectedly small: {}",
        admin.len()
    );
    let wallet_token = app.state.jwt.issue(WALLET_A)?;
    let mut failures = Vec::new();
    for (method, path) in &admin {
        let attempts: [(&str, Vec<(&str, String)>); 4] = [
            ("no credentials", vec![]),
            ("wrong key", vec![("x-api-key", "not-the-key".into())]),
            ("empty key", vec![("x-api-key", String::new())]),
            ("wallet JWT only", vec![bearer(&wallet_token)]),
        ];
        for (label, headers) in attempts {
            let status = call(&app, method, path, &headers).await?;
            if status != StatusCode::UNAUTHORIZED {
                failures.push(format!("{method} {path}: {label} -> {status}"));
            }
        }
    }
    // Positive control on cheap read-only routes only: a valid key must get past the gate.
    // Mutating admin handlers are not run here (they reach for the chain).
    for path in ["/admin/audit-log", "/admin/db/stats", "/admin/tasks/status"] {
        let status = call(&app, "GET", path, &[("x-api-key", "dev".into())]).await?;
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            failures.push(format!(
                "GET {path}: the configured key was rejected ({status})"
            ));
        }
    }
    anyhow::ensure!(failures.is_empty(), "admin gate failures: {failures:#?}");
    Ok(())
}

#[tokio::test]
async fn wallet_routes_reject_bad_tokens_and_accept_a_good_one() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let table = table();
    let routes = runtime_routes(&app).await?;
    let wallet_routes = routes_of(Wallet, &routes, &table);
    anyhow::ensure!(
        wallet_routes.len() >= 30,
        "wallet surface unexpectedly small"
    );
    let wrong_secret = forged_token("some-other-secret", WALLET_A, 3600)?;
    let expired = forged_token(JWT_SECRET, WALLET_A, -3600)?;
    let mut failures = Vec::new();
    for (index, (method, path)) in wallet_routes.iter().enumerate() {
        let attempts: [(&str, Vec<(&str, String)>); 5] = [
            ("no credentials", vec![]),
            ("malformed bearer", vec![bearer("not-a-jwt")]),
            ("wrong signing secret", vec![bearer(&wrong_secret)]),
            ("expired token", vec![bearer(&expired)]),
            (
                "api key instead of a token",
                vec![("x-api-key", "dev".into())],
            ),
        ];
        for (label, headers) in attempts {
            let status = call(&app, method, path, &headers).await?;
            if status != StatusCode::UNAUTHORIZED {
                failures.push(format!("{method} {path}: {label} -> {status}"));
            }
        }
        // A fresh subject per route: `/api/auth/logout` revokes every earlier token for its
        // wallet, which would otherwise make the routes after it look like gate failures.
        let valid = app.state.jwt.issue(&format!("probe-wallet-{index}"))?;
        let status = call(&app, method, path, &[bearer(&valid)]).await?;
        // These handlers answer 401 "Account not found" for a wallet with no profile.
        let handler_401 = matches!(
            (method.as_str(), path.as_str()),
            ("GET", "/api/auth/me") | ("POST", "/api/auth/sync-profile")
        );
        if status == StatusCode::UNAUTHORIZED && !handler_401 {
            failures.push(format!("{method} {path}: a valid token was rejected"));
        }
    }
    anyhow::ensure!(failures.is_empty(), "wallet gate failures: {failures:#?}");
    Ok(())
}

#[tokio::test]
async fn logout_revokes_tokens_issued_before_it() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    // Back-dated so it predates the logout second (revocation is strict `iat < cutoff`).
    let token = forged_token(JWT_SECRET, WALLET_A, 3600)?;
    let probe = || call_with_token(&app, "POST", "/telemetry/blur", &token);
    assert_ne!(
        probe().await?,
        StatusCode::UNAUTHORIZED,
        "token is valid before logout"
    );
    assert_eq!(
        call_with_token(&app, "POST", "/api/auth/logout", &token).await?,
        StatusCode::OK
    );
    assert_eq!(
        probe().await?,
        StatusCode::UNAUTHORIZED,
        "token is dead after logout"
    );
    Ok(())
}

async fn call_with_token(
    app: &common::TestApp,
    method: &str,
    path: &str,
    token: &str,
) -> anyhow::Result<StatusCode> {
    call(app, method, path, &[bearer(token)]).await
}

#[tokio::test]
async fn a_wallet_token_cannot_act_for_another_wallet() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let token_for_b = app.state.jwt.issue(WALLET_B)?;
    let status = call(
        &app,
        "DELETE",
        &format!("/api/global-session/{WALLET_A}"),
        &[bearer(&token_for_b)],
    )
    .await?;
    assert_eq!(status, StatusCode::FORBIDDEN, "cross-wallet session delete");
    Ok(())
}

#[tokio::test]
async fn handler_routes_never_succeed_anonymously() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let table = table();
    let routes = runtime_routes(&app).await?;
    let mut failures = Vec::new();
    for (method, path) in routes_of(Handler, &routes, &table) {
        // `{}` is rarely a valid body, so this proves only that no empty request is accepted.
        let status = call(&app, &method, &path, &[]).await?;
        if status.is_success() || status.is_server_error() {
            failures.push(format!("{method} {path} -> {status}"));
        }
    }
    anyhow::ensure!(
        failures.is_empty(),
        "anonymous request accepted: {failures:#?}"
    );
    Ok(())
}

async fn call_json(
    app: &common::TestApp,
    method: &str,
    path: &str,
    token: &str,
    body: serde_json::Value,
) -> anyhow::Result<(StatusCode, String)> {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))?;
    let response = app.router().oneshot(request).await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

/// Routes that name a wallet in the body must reject a token for any other wallet.
#[tokio::test]
async fn routes_naming_a_wallet_in_the_body_reject_other_wallets() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let token_for_b = app.state.jwt.issue(WALLET_B)?;
    let cases = [
        (
            "/api/external-elo/link/start",
            serde_json::json!({ "pubkey": WALLET_A, "username": "someone" }),
        ),
        (
            "/api/external-elo/sync",
            serde_json::json!({ "pubkey": WALLET_A }),
        ),
        (
            "/dispute/notify",
            serde_json::json!({
                "game_id": 1,
                "challenger_wallet": WALLET_A,
                "reason": "x",
                "tx_signature": "sig",
            }),
        ),
        (
            "/api/tournament/1/subscribe-node",
            serde_json::json!({ "player": WALLET_A, "node_id": "node" }),
        ),
        (
            "/tournament/1/subscribe-node",
            serde_json::json!({ "player": WALLET_A, "node_id": "node" }),
        ),
    ];
    for (path, body) in cases {
        let (status, text) = call_json(&app, "POST", path, &token_for_b, body).await?;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {text}");
    }
    Ok(())
}

/// A link started by one wallet cannot be confirmed (or consumed) by another.
#[tokio::test]
async fn another_wallet_cannot_confirm_a_started_link() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let token_a = app.state.jwt.issue(WALLET_A)?;
    let token_b = app.state.jwt.issue(WALLET_B)?;
    let (status, text) = call_json(
        &app,
        "POST",
        "/api/external-elo/link/start",
        &token_a,
        serde_json::json!({ "pubkey": WALLET_A, "username": "someone" }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    let link_id = serde_json::from_str::<serde_json::Value>(&text)?["link_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("link_start returned no link_id: {text}"))?;

    for _ in 0..2 {
        let (status, text) = call_json(
            &app,
            "POST",
            "/api/external-elo/link/confirm",
            &token_b,
            serde_json::json!({ "link_id": link_id }),
        )
        .await?;
        assert_eq!(status, StatusCode::FORBIDDEN, "{text}");
    }
    Ok(())
}

const NODE_HEADER: &str = "x-node-auth";

#[tokio::test]
async fn node_routes_require_a_valid_node_token() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let table = table();
    let routes = runtime_routes(&app).await?;
    let node_routes = routes_of(Node, &routes, &table);
    anyhow::ensure!(node_routes.len() >= 14, "node surface unexpectedly small");

    let (owner, thief) = (Keypair::new(), Keypair::new());
    let owner_id = owner.pubkey().to_string();
    // The thief signs a token that names the owner's node id.
    let issued_at = chrono::Utc::now().timestamp();
    let forged = format!(
        "v1.{owner_id}.{issued_at}.{}",
        thief.sign_message(backend::signing::node_auth::signable(&owner_id, issued_at).as_bytes())
    );
    let expired = common::node_token_aged(&owner, backend::signing::node_auth::MAX_AGE_SECS + 60);
    let wallet_token = app.state.jwt.issue(WALLET_A)?;
    let mut failures = Vec::new();
    for (method, path) in &node_routes {
        let attempts: [(&str, Vec<(&str, String)>); 5] = [
            ("no credentials", vec![]),
            ("garbage token", vec![(NODE_HEADER, "not-a-token".into())]),
            ("expired token", vec![(NODE_HEADER, expired.clone())]),
            (
                "another node signature",
                vec![(NODE_HEADER, forged.clone())],
            ),
            // A wallet token proves a wallet, not a node.
            (
                "wallet token instead of a node token",
                vec![bearer(&wallet_token)],
            ),
        ];
        for (label, headers) in attempts {
            let status = call(&app, method, path, &headers).await?;
            if status != StatusCode::UNAUTHORIZED {
                failures.push(format!("{method} {path}: {label} -> {status}"));
            }
        }
        let status = call(
            &app,
            method,
            path,
            &[(NODE_HEADER, common::node_token(&owner))],
        )
        .await?;
        if status == StatusCode::UNAUTHORIZED {
            failures.push(format!("{method} {path}: a valid node token was rejected"));
        }
    }
    anyhow::ensure!(failures.is_empty(), "node gate failures: {failures:#?}");
    Ok(())
}

/// Every node id a request names must be the one that signed.
#[tokio::test]
async fn node_routes_reject_a_node_acting_for_another() -> anyhow::Result<()> {
    use serde_json::json;
    let app = common::spawn_app().await?;
    let (me, other) = (Keypair::new(), Keypair::new());
    let other_id = other.pubkey().to_string();
    let cases = [
        (
            "POST",
            "/p2p/announce".to_string(),
            Some(json!({
                "game_id": "g", "host_node_id": other_id, "display_name": "x",
                "stake_amount": 0.0, "game_type": "P2P", "base_time_seconds": 300,
                "increment_seconds": 0, "username": null, "elo": null,
                "region": null, "password": null
            })),
        ),
        (
            "POST",
            "/p2p/join".to_string(),
            Some(json!({ "game_id": "g", "joiner_node_id": other_id, "password": null })),
        ),
        (
            "POST",
            "/p2p/accept".to_string(),
            Some(json!({ "game_id": "g", "host_node_id": other_id })),
        ),
        (
            "POST",
            "/p2p/leave".to_string(),
            Some(json!({ "game_id": "g", "node_id": other_id })),
        ),
        (
            "POST",
            "/p2p/heartbeat".to_string(),
            Some(json!({ "game_id": "g", "host_node_id": other_id })),
        ),
        (
            "POST",
            "/p2p/poll".to_string(),
            Some(json!({ "game_id": "g", "node_id": other_id, "since_index": 0 })),
        ),
        (
            "POST",
            "/friends/requests".to_string(),
            Some(json!({
                "from_node_id": other_id, "from_pubkey": null, "from_display": "x",
                "to_node_id": "someone", "to_pubkey": null, "message": null
            })),
        ),
        ("GET", format!("/friends/requests?node_id={other_id}"), None),
        (
            "PUT",
            "/friends/requests/1".to_string(),
            Some(json!({ "action": "accept", "responder_node_id": other_id })),
        ),
        ("GET", format!("/friends?node_id={other_id}"), None),
        (
            "DELETE",
            format!("/friends/{WALLET_A}?node_id={other_id}"),
            None,
        ),
        (
            "PUT",
            "/presence".to_string(),
            Some(json!({
                "node_id": other_id, "pubkey": null, "display_name": "x",
                "status": "online", "game_id": null, "updated_at": chrono::Utc::now()
            })),
        ),
        (
            "POST",
            "/friends/invite".to_string(),
            Some(json!({
                "game_id": "g", "from_node_id": other_id, "from_display": "x", "to_node_id": "t"
            })),
        ),
        (
            "GET",
            format!("/social/poll?node_id={other_id}&since_index=0"),
            None,
        ),
    ];
    for (method, path, body) in cases {
        let (status, text) = app.as_node(&me, method, &path, body.as_ref()).await?;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {text}");
    }
    Ok(())
}

/// Only the addressee may answer a friend request, and a wallet named in a social call counts
/// only when the bearer token is for that wallet.
#[tokio::test]
async fn friend_requests_belong_to_their_addressee() -> anyhow::Result<()> {
    use serde_json::json;
    let app = common::spawn_app().await?;
    let (alice, bob, mallory) = (Keypair::new(), Keypair::new(), Keypair::new());
    let (alice_id, bob_id, mallory_id) = (
        alice.pubkey().to_string(),
        bob.pubkey().to_string(),
        mallory.pubkey().to_string(),
    );
    let (status, request) = app
        .as_node(
            &alice,
            "POST",
            "/friends/requests",
            Some(&json!({
                "from_node_id": alice_id, "from_pubkey": WALLET_B, "from_display": "alice",
                "to_node_id": bob_id, "to_pubkey": null, "message": null
            })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "{request}");
    // A wallet claimed without that wallet's token is dropped, not recorded.
    assert!(request["from_pubkey"].is_null(), "{request}");
    let id = request["id"].as_str().unwrap_or_default().to_string();
    anyhow::ensure!(!id.is_empty(), "no request id: {request}");

    let path = format!("/friends/requests/{id}");
    for (who, who_id, expected, label) in [
        (&mallory, &mallory_id, StatusCode::FORBIDDEN, "a stranger"),
        (&alice, &alice_id, StatusCode::FORBIDDEN, "the sender"),
        (&bob, &bob_id, StatusCode::OK, "the addressee"),
    ] {
        let body = json!({ "action": "accept", "responder_node_id": who_id });
        let (status, text) = app.as_node(who, "PUT", &path, Some(&body)).await?;
        assert_eq!(status, expected, "{label}: {text}");
    }
    Ok(())
}

/// A request addressed to a wallet is visible only to a caller holding that wallet's token.
#[tokio::test]
async fn requests_addressed_to_a_wallet_need_that_wallets_token() -> anyhow::Result<()> {
    use serde_json::json;
    let app = common::spawn_app().await?;
    let (sender, reader) = (Keypair::new(), Keypair::new());
    let (status, text) = app
        .as_node(
            &sender,
            "POST",
            "/friends/requests",
            Some(&json!({
                "from_node_id": sender.pubkey().to_string(), "from_pubkey": null,
                "from_display": "s", "to_node_id": null, "to_pubkey": WALLET_A, "message": null
            })),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "{text}");

    let path = format!(
        "/friends/requests?node_id={}&pubkey={WALLET_A}",
        reader.pubkey()
    );
    let (_, without) = app.as_node(&reader, "GET", &path, None).await?;
    assert_eq!(without.as_array().map(Vec::len), Some(0), "{without}");

    let token = app.state.jwt.issue(WALLET_A)?;
    let request = Request::builder()
        .method("GET")
        .uri(&path)
        .header(NODE_HEADER, common::node_token(&reader))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let (status, with) = app.send(request).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(with.as_array().map(Vec::len), Some(1), "{with}");
    Ok(())
}

/// A relay sender must prove the identity it writes as.
#[tokio::test]
async fn game_writes_must_prove_the_sender() -> anyhow::Result<()> {
    use serde_json::json;
    let app = common::spawn_app().await?;
    let (host, other) = (Keypair::new(), Keypair::new());
    let host_id = host.pubkey().to_string();
    let chat = json!({
        "player_pubkey": host_id,
        "session_token": "",
        "message": { "type": "chat", "player": host_id, "text": "hi", "timestamp_ms": 1 },
        "content_version": "",
        "content_parent": "",
    });
    let path = "/game/chat-proof/chat";
    let anonymous = Request::builder()
        .method("PUT")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(chat.to_string()))?;
    assert_eq!(app.send(anonymous).await?.0, StatusCode::UNAUTHORIZED);
    // Another node's token does not vouch for the named sender.
    assert_eq!(
        app.as_node(&other, "PUT", path, Some(&chat)).await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_ne!(
        app.as_node(&host, "PUT", path, Some(&chat)).await?.0,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

/// Broadcasting spends our RPC quota: signed-in users only, checked before the payload.
#[tokio::test]
async fn broadcast_tx_requires_a_signed_in_user() -> anyhow::Result<()> {
    use serde_json::json;
    let app = common::spawn_app().await?;
    let body = json!({ "tx_b64": "AA==" });
    let anonymous = Request::builder()
        .method("POST")
        .uri("/api/auth/broadcast-tx")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))?;
    assert_eq!(app.send(anonymous).await?.0, StatusCode::UNAUTHORIZED);
    let token = app.state.jwt.issue(WALLET_A)?;
    let (status, text) = call_json(&app, "POST", "/api/auth/broadcast-tx", &token, body).await?;
    // Past the gate: the payload is rejected as a transaction instead.
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    Ok(())
}
