//! Profile principals use the existing signed node-token protocol, while game
//! transport ownership is proved separately. No live backend or chain is used.
mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use solana_sdk::{signature::Keypair, signer::Signer};

async fn send_request(
    app: &common::TestApp,
    from: &Keypair,
    to: &Keypair,
) -> anyhow::Result<Value> {
    let (status, body) = app.as_node(from, "POST", "/friends/requests", Some(&json!({
        "from_node_id": from.pubkey().to_string(), "from_display": "Alice", "to_node_id": to.pubkey().to_string()
    }))).await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {body}");
    Ok(body)
}

async fn accept(app: &common::TestApp, request: &Value, recipient: &Keypair) -> anyhow::Result<()> {
    let (status, body) = app.as_node(recipient, "PUT", &format!("/friends/requests/{}", request["id"].as_str().unwrap()), Some(&json!({
        "responder_node_id": recipient.pubkey().to_string(), "responder_display": "Bob", "action": "accept"
    }))).await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {body}");
    Ok(())
}

#[tokio::test]
async fn profiles_are_isolated_and_friend_lifecycle_can_repeat() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice, bob, second_profile) = (Keypair::new(), Keypair::new(), Keypair::new());
    let request = send_request(&app, &alice, &bob).await?;
    let request_id = request["id"].as_str().unwrap();
    let outgoing = format!("/friends/requests?node_id={}&outgoing=true", alice.pubkey());
    assert_eq!(
        app.as_node(&alice, "GET", &outgoing, None)
            .await?
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // A second profile on the same machine cannot read or mutate the first graph.
    assert_eq!(
        app.as_node(&second_profile, "GET", &outgoing, None)
            .await?
            .0,
        StatusCode::FORBIDDEN
    );
    let wrong_cancel = format!(
        "/friends/requests/{request_id}?node_id={}",
        second_profile.pubkey()
    );
    assert_eq!(
        app.as_node(&second_profile, "DELETE", &wrong_cancel, None)
            .await?
            .0,
        StatusCode::FORBIDDEN
    );
    accept(&app, &request, &bob).await?;
    let contacts = format!("/friends?node_id={}", alice.pubkey());
    let alice_contacts = app.as_node(&alice, "GET", &contacts, None).await?.1;
    assert_eq!(alice_contacts[0]["contact_display"], "Bob");
    assert_eq!(
        app.as_node(
            &second_profile,
            "GET",
            &format!("/friends?node_id={}", second_profile.pubkey()),
            None
        )
        .await?
        .1,
        json!([])
    );
    let remove = format!("/friends/{}?node_id={}", bob.pubkey(), alice.pubkey());
    assert_eq!(
        app.as_node(&alice, "DELETE", &remove, None).await?.0,
        StatusCode::OK
    );
    let request = send_request(&app, &alice, &bob).await?;
    let (_, _) = app
        .as_node(
            &bob,
            "PUT",
            &format!("/friends/requests/{}", request["id"].as_str().unwrap()),
            Some(&json!({
                "responder_node_id": bob.pubkey().to_string(), "action": "reject"
            })),
        )
        .await?;
    // The old unique constraints used to prevent this re-request after rejection.
    let request = send_request(&app, &alice, &bob).await?;
    accept(&app, &request, &bob).await?;
    assert_eq!(
        app.as_node(&alice, "GET", &contacts, None)
            .await?
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
    Ok(())
}

async fn profile_presence(
    app: &common::TestApp,
    profile: &Keypair,
    transport: &Keypair,
    game: Option<&str>,
) -> anyhow::Result<()> {
    let request = Request::builder().method("PUT").uri("/presence")
        .header("content-type", "application/json")
        .header("x-node-auth", common::node_token(profile))
        .header("x-transport-auth", common::node_token(transport))
        .body(Body::from(json!({ "node_id": profile.pubkey().to_string(), "display_name": "Profile", "status": "online", "game_id": game, "updated_at": chrono::Utc::now() }).to_string()))?;
    let (status, body) = app.send(request).await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {body}");
    Ok(())
}

async fn invite(
    app: &common::TestApp,
    profile: &Keypair,
    transport: &Keypair,
    recipient: &Keypair,
    game: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    app.send(Request::builder().method("POST").uri("/friends/invite")
        .header("content-type", "application/json")
        .header("x-node-auth", common::node_token(profile))
        .header("x-transport-auth", common::node_token(transport))
        .body(Body::from(json!({ "game_id": game, "from_node_id": profile.pubkey().to_string(), "from_display": "Alice", "to_node_id": recipient.pubkey().to_string(), "direct": true }).to_string()))?).await
}

#[tokio::test]
async fn invitations_are_owned_durable_deduplicated_and_close_with_host() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice, bob, transport, stranger) = (
        Keypair::new(),
        Keypair::new(),
        Keypair::new(),
        Keypair::new(),
    );
    assert_eq!(
        invite(&app, &stranger, &transport, &bob, "room").await?.0,
        StatusCode::FORBIDDEN
    );
    let request = send_request(&app, &alice, &bob).await?;
    accept(&app, &request, &bob).await?;
    profile_presence(&app, &alice, &transport, Some("room")).await?;
    for _ in 0..2 {
        assert_eq!(
            invite(&app, &alice, &transport, &bob, "room").await?.0,
            StatusCode::OK
        );
    }
    let poll = format!("/social/poll?node_id={}&since_index=999999", bob.pubkey());
    let inbox = app.as_node(&bob, "GET", &poll, None).await?.1;
    assert_eq!(
        inbox["invites"].as_array().unwrap().len(),
        1,
        "Old array cursors must never hide new invites"
    );
    let id = inbox["invites"][0]["id"].as_str().unwrap();
    assert_eq!(
        inbox["invites"][0]["direct_node_id"],
        transport.pubkey().to_string()
    );
    // A different signed identity cannot dismiss somebody else's inbox.
    let wrong = format!("/friends/invites/{id}?node_id={}", bob.pubkey());
    assert_eq!(
        app.as_node(&stranger, "DELETE", &wrong, None).await?.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.state
            .friends
            .invites(&bob.pubkey().to_string())
            .await?
            .len(),
        1
    );
    profile_presence(&app, &alice, &transport, None).await?;
    assert_eq!(
        app.as_node(&bob, "GET", &poll, None).await?.1["invites"],
        json!([])
    );
    assert_eq!(
        app.as_node(&bob, "DELETE", &wrong, None).await?.0,
        StatusCode::OK
    );
    assert!(app
        .state
        .friends
        .invites(&bob.pubkey().to_string())
        .await?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn switching_profiles_retires_previous_presence_on_the_same_transport() -> anyhow::Result<()>
{
    let app = common::spawn_app().await?;
    let (a, b, transport) = (Keypair::new(), Keypair::new(), Keypair::new());
    profile_presence(&app, &a, &transport, None).await?;
    assert!(app
        .state
        .presence
        .get_online(&a.pubkey().to_string())
        .is_some());
    profile_presence(&app, &b, &transport, None).await?;
    assert!(app
        .state
        .presence
        .get_online(&a.pubkey().to_string())
        .is_none());
    assert!(app
        .state
        .presence
        .get_online(&b.pubkey().to_string())
        .is_some());
    // Profile aliases are not counted as additional players.
    assert_eq!(app.state.presence.count_online(), 0);
    Ok(())
}

#[tokio::test]
async fn invitation_storage_survives_manager_recreation_caps_and_expiry() -> anyhow::Result<()> {
    use backend::signing::{social::routes::LobbyInvite, FriendManager};
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await?;
    sqlx::raw_sql(include_str!("../migrations/session/015_social_invites.sql")).execute(&pool).await?;
    let store = FriendManager::new(pool.clone());
    for index in 0..55 {
        store.save_invite("recipient", &LobbyInvite { id: index.to_string(), game_id: format!("room-{index}"), from_node_id: "sender".into(), from_display: "Alice".into(), direct_node_id: None, received_at: chrono::Utc::now() }).await?;
    }
    let reopened = FriendManager::new(pool.clone());
    let inbox = reopened.invites("recipient").await?;
    assert_eq!(inbox.len(), 50);
    assert_eq!(inbox[0].id, "54");
    reopened.dismiss_invite("stranger", "54").await?;
    assert_eq!(reopened.invites("recipient").await?.len(), 50);
    reopened.dismiss_invite("recipient", "54").await?;
    assert_eq!(reopened.invites("recipient").await?.len(), 49);
    sqlx::query("UPDATE social_invites SET expires_at = 0").execute(&pool).await?;
    assert!(reopened.invites("recipient").await?.is_empty());
    Ok(())
}

/// A request as `node`, carrying the bearer token of `wallet` when there is one.
async fn call_as(
    app: &common::TestApp,
    node: &Keypair,
    wallet: Option<&Keypair>,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> anyhow::Result<(StatusCode, Value)> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-node-auth", common::node_token(node));
    if let Some(wallet) = wallet {
        let token = app.state.jwt.issue(&wallet.pubkey().to_string())?;
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))?,
        None => builder.body(Body::empty())?,
    };
    app.send(request).await
}

async fn announce(app: &common::TestApp, node: &Keypair, wallet: &Keypair) -> anyhow::Result<()> {
    let (status, body) = call_as(
        app,
        node,
        Some(wallet),
        "PUT",
        "/presence",
        Some(json!({
            "node_id": node.pubkey().to_string(), "pubkey": wallet.pubkey().to_string(),
            "display_name": "Player", "status": "online", "game_id": null,
            "updated_at": chrono::Utc::now()
        })),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {body}");
    Ok(())
}

async fn wallet_request(
    app: &common::TestApp,
    from: (&Keypair, &Keypair),
    to_node: &Keypair,
) -> anyhow::Result<(StatusCode, Value)> {
    call_as(
        app,
        from.0,
        Some(from.1),
        "POST",
        "/friends/requests",
        Some(json!({
            "from_node_id": from.0.pubkey().to_string(),
            "from_pubkey": from.1.pubkey().to_string(), "from_display": "Alice",
            "to_node_id": to_node.pubkey().to_string()
        })),
    )
    .await
}

async fn wallet_accept(
    app: &common::TestApp,
    request: &Value,
    node: &Keypair,
    wallet: &Keypair,
) -> anyhow::Result<()> {
    let (status, body) = call_as(
        app,
        node,
        Some(wallet),
        "PUT",
        &format!("/friends/requests/{}", request["id"].as_str().unwrap()),
        Some(json!({
            "responder_node_id": node.pubkey().to_string(), "responder_display": "Bob", "action": "accept"
        })),
    )
    .await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {body}");
    Ok(())
}

#[tokio::test]
async fn wallet_friendships_follow_the_wallet_to_a_new_device() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice_wallet, bob_wallet) = (Keypair::new(), Keypair::new());
    let (alice_phone, alice_laptop, bob_node, transport) = (
        Keypair::new(),
        Keypair::new(),
        Keypair::new(),
        Keypair::new(),
    );
    announce(&app, &alice_phone, &alice_wallet).await?;
    announce(&app, &bob_node, &bob_wallet).await?;
    let (status, request) = wallet_request(&app, (&alice_phone, &alice_wallet), &bob_node).await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {request}");
    wallet_accept(&app, &request, &bob_node, &bob_wallet).await?;

    // Alice signs in on a device the graph has never seen: the friend is already there.
    announce(&app, &alice_laptop, &alice_wallet).await?;
    let path = format!("/friends?node_id={}", alice_laptop.pubkey());
    let (_, laptop) = call_as(&app, &alice_laptop, Some(&alice_wallet), "GET", &path, None).await?;
    assert_eq!(laptop.as_array().map(Vec::len), Some(1), "{laptop}");
    assert_eq!(laptop[0]["contact_node_id"], bob_node.pubkey().to_string());
    // Without the wallet token another new device has no claim on the wallet friends.
    let tablet = Keypair::new();
    announce(&app, &tablet, &alice_wallet).await?;
    let tablet_path = format!("/friends?node_id={}", tablet.pubkey());
    let (_, anonymous) = call_as(&app, &tablet, None, "GET", &tablet_path, None).await?;
    assert_eq!(anonymous, json!([]));
    // The laptop contact exists because the wallet synced it; routing now points the tablet at Bob.
    let (_, tablet_list) = call_as(&app, &tablet, Some(&alice_wallet), "GET", &tablet_path, None).await?;
    assert_eq!(tablet_list.as_array().map(Vec::len), Some(1), "{tablet_list}");

    // Bob contact for Alice follows her to her newest device the next time he looks.
    let bob_path = format!("/friends?node_id={}", bob_node.pubkey());
    let (_, bob_list) = call_as(&app, &bob_node, Some(&bob_wallet), "GET", &bob_path, None).await?;
    assert_eq!(bob_list.as_array().map(Vec::len), Some(1), "{bob_list}");
    assert_eq!(bob_list[0]["contact_node_id"], tablet.pubkey().to_string());

    // Wallet friends can invite one another without a node-level request on this device.
    profile_presence(&app, &alice_laptop, &transport, Some("room")).await?;
    let (status, body) = call_as(
        &app,
        &alice_laptop,
        Some(&alice_wallet),
        "POST",
        "/friends/invite",
        Some(json!({
            "game_id": "room", "from_node_id": alice_laptop.pubkey().to_string(),
            "from_display": "Alice", "to_node_id": bob_node.pubkey().to_string(), "direct": false
        })),
    )
    .await?;
    // Friendship is accepted; the lobby itself is what is missing here.
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "This lobby has closed");

    // Removing the friend removes the wallet relationship, so it does not come back.
    let remove = format!("/friends/{}?node_id={}", bob_node.pubkey(), alice_laptop.pubkey());
    assert_eq!(
        call_as(&app, &alice_laptop, Some(&alice_wallet), "DELETE", &remove, None).await?.0,
        StatusCode::OK
    );
    let (_, after) = call_as(&app, &alice_laptop, Some(&alice_wallet), "GET", &path, None).await?;
    assert_eq!(after, json!([]));
    assert!(
        !app.state
            .friends
            .wallets_are_friends(&alice_wallet.pubkey().to_string(), &bob_wallet.pubkey().to_string())
            .await?
    );
    Ok(())
}

#[tokio::test]
async fn blocking_removes_the_friendship_and_stops_requests_and_invitations() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice, bob, transport) = (Keypair::new(), Keypair::new(), Keypair::new());
    let request = send_request(&app, &alice, &bob).await?;
    accept(&app, &request, &bob).await?;
    profile_presence(&app, &alice, &transport, Some("room")).await?;
    assert_eq!(invite(&app, &alice, &transport, &bob, "room").await?.0, StatusCode::OK);

    let block = json!({
        "blocker_node_id": bob.pubkey().to_string(), "blocked_node_id": alice.pubkey().to_string()
    });
    // Only the blocker own node can block on its behalf.
    assert_eq!(
        app.as_node(&alice, "POST", "/friends/blocks", Some(&block)).await?.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.as_node(&bob, "POST", "/friends/blocks", Some(&block)).await?.0,
        StatusCode::OK
    );
    // The friendship and the invitation Bob already held are gone.
    for node in [&alice, &bob] {
        let list = app.as_node(node, "GET", &format!("/friends?node_id={}", node.pubkey()), None).await?.1;
        assert_eq!(list, json!([]));
    }
    assert!(app.state.friends.invites(&bob.pubkey().to_string()).await?.is_empty());

    // Alice can neither re-request nor invite; Bob cannot request her without unblocking.
    let (status, body) = app.as_node(&alice, "POST", "/friends/requests", Some(&json!({
        "from_node_id": alice.pubkey().to_string(), "from_display": "Alice", "to_node_id": bob.pubkey().to_string()
    }))).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = app.as_node(&bob, "POST", "/friends/requests", Some(&json!({
        "from_node_id": bob.pubkey().to_string(), "from_display": "Bob", "to_node_id": alice.pubkey().to_string()
    }))).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(invite(&app, &alice, &transport, &bob, "room").await?.0, StatusCode::FORBIDDEN);

    let listed = app.as_node(&bob, "GET", &format!("/friends/blocks?node_id={}", bob.pubkey()), None).await?.1;
    assert_eq!(listed[0]["blocked_node_id"], alice.pubkey().to_string());

    let lift = format!("/friends/blocks/{}?node_id={}", alice.pubkey(), bob.pubkey());
    assert_eq!(app.as_node(&bob, "DELETE", &lift, None).await?.0, StatusCode::OK);
    let request = send_request(&app, &alice, &bob).await?;
    accept(&app, &request, &bob).await?;
    Ok(())
}

#[tokio::test]
async fn a_block_holds_against_the_blocked_wallet_on_another_device() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice_wallet, bob_wallet) = (Keypair::new(), Keypair::new());
    let (alice_old, alice_new, bob_node) = (Keypair::new(), Keypair::new(), Keypair::new());
    announce(&app, &alice_old, &alice_wallet).await?;
    announce(&app, &bob_node, &bob_wallet).await?;
    let (status, request) = wallet_request(&app, (&alice_old, &alice_wallet), &bob_node).await?;
    anyhow::ensure!(status == StatusCode::OK, "{status}: {request}");
    wallet_accept(&app, &request, &bob_node, &bob_wallet).await?;
    let (status, body) = call_as(
        &app,
        &bob_node,
        Some(&bob_wallet),
        "POST",
        "/friends/blocks",
        Some(json!({
            "blocker_node_id": bob_node.pubkey().to_string(),
            "blocked_node_id": alice_old.pubkey().to_string()
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !app.state
            .friends
            .wallets_are_friends(&alice_wallet.pubkey().to_string(), &bob_wallet.pubkey().to_string())
            .await?
    );

    // Alice returns from a device Bob has never blocked; her wallet is still blocked.
    announce(&app, &alice_new, &alice_wallet).await?;
    let (status, body) = wallet_request(&app, (&alice_new, &alice_wallet), &bob_node).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    Ok(())
}

async fn request_by_name(
    app: &common::TestApp,
    node: &Keypair,
    wallet: &Keypair,
    username: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    call_as(
        app,
        node,
        Some(wallet),
        "POST",
        "/friends/requests",
        Some(json!({
            "from_node_id": node.pubkey().to_string(),
            "from_pubkey": wallet.pubkey().to_string(), "from_display": "Alice",
            "to_username": username
        })),
    )
    .await
}

#[tokio::test]
async fn friends_are_added_by_username_and_the_wallet_sees_the_request() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (alice_wallet, bob_wallet) = (Keypair::new(), Keypair::new());
    let (alice, bob) = (Keypair::new(), Keypair::new());
    app.state
        .store
        .create_wallet_user(&bob_wallet.pubkey().to_string(), "BobChess", None)
        .await?;
    announce(&app, &alice, &alice_wallet).await?;
    announce(&app, &bob, &bob_wallet).await?;

    // Case does not matter, and the sender sees the name rather than a key.
    let (status, request) = request_by_name(&app, &alice, &alice_wallet, "  bobchess ").await?;
    assert_eq!(status, StatusCode::OK, "{request}");
    assert_eq!(request["to_pubkey"], bob_wallet.pubkey().to_string());
    assert_eq!(request["to_display"], "BobChess");
    let outgoing = format!("/friends/requests?node_id={}&outgoing=true", alice.pubkey());
    let (_, sent) = call_as(&app, &alice, Some(&alice_wallet), "GET", &outgoing, None).await?;
    assert_eq!(sent[0]["to_display"], "BobChess");

    // The request is addressed to a wallet, so Bob sees it once his client names that wallet.
    let incoming = format!(
        "/friends/requests?node_id={}&pubkey={}",
        bob.pubkey(),
        bob_wallet.pubkey()
    );
    let (_, seen) = call_as(&app, &bob, Some(&bob_wallet), "GET", &incoming, None).await?;
    assert_eq!(seen.as_array().map(Vec::len), Some(1), "{seen}");
    let blind = format!("/friends/requests?node_id={}", bob.pubkey());
    let (_, unseen) = call_as(&app, &bob, Some(&bob_wallet), "GET", &blind, None).await?;
    assert_eq!(unseen, json!([]));

    wallet_accept(&app, &seen[0], &bob, &bob_wallet).await?;
    let (status, body) = request_by_name(&app, &alice, &alice_wallet, "BobChess").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "You are already friends");

    // Unknown names and ambiguous targets are refused.
    let (status, body) = request_by_name(&app, &alice, &alice_wallet, "nobody-here").await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _) = call_as(
        &app,
        &alice,
        Some(&alice_wallet),
        "POST",
        "/friends/requests",
        Some(json!({
            "from_node_id": alice.pubkey().to_string(), "from_display": "Alice",
            "to_username": "BobChess", "to_node_id": bob.pubkey().to_string()
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn username_lookups_are_rate_limited_per_node() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (wallet, node) = (Keypair::new(), Keypair::new());
    let mut limited = false;
    for _ in 0..12 {
        let (status, _) = request_by_name(&app, &node, &wallet, "nobody-here").await?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            limited = true;
            break;
        }
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    assert!(limited, "a node could probe usernames without limit");
    Ok(())
}
