#![cfg(feature = "proxy")]

use braid_chess::{ChessMessage, ChessPublisher, ChessSubscriber, MovePayload};
use braid_http::{BraidClient, BraidRequest, Update, Version};
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig, ProxyConfig};
use futures::StreamExt;
use std::time::Duration;

#[tokio::test]
async fn game_updates_cross_gossip_topics_between_two_local_peers() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let discovery = DiscoveryConfig::mock();
        let a = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
        let b = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
        discovery.add_node(a.node_addr().await.unwrap());
        discovery.add_node(b.node_addr().await.unwrap());
        let mut a_events = a.subscribe("game/9/moves/", vec![]).await.unwrap();
        let mut b_events = b
            .subscribe("/game/9/moves", vec![a.node_id()])
            .await
            .unwrap();
        while !matches!(
            a_events.next().await.unwrap().unwrap(),
            iroh_gossip::api::Event::NeighborUp(_)
        ) {}
        let update = Update::snapshot(Version::new("move-1"), "e2e4");
        a.put("game/9/moves", update.clone()).await.unwrap();
        loop {
            if let iroh_gossip::api::Event::Received(message) =
                b_events.next().await.unwrap().unwrap()
            {
                let received: Update = serde_json::from_slice(&message.content).unwrap();
                assert_eq!(received.version, update.version);
                assert_eq!(received.body_str(), Some("e2e4"));
                break;
            }
        }
        a.shutdown().await.unwrap();
        b.shutdown().await.unwrap();
    })
    .await
    .expect("gossip game update stalled");
}

#[tokio::test]
async fn desktop_bridge_delivers_history_live_moves_and_independent_chat_parents() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let key = iroh::SecretKey::generate();
        let node = BraidIrohNode::spawn(BraidIrohConfig {
            secret_key: Some(key.clone()),
            proxy_config: Some(ProxyConfig {
                listen_addr: "127.0.0.1:0".parse().unwrap(),
                default_peer: key.public(),
            }),
            ..Default::default()
        })
        .await
        .unwrap();
        let base = format!("http://{}", node.proxy_addr().unwrap());
        let mut publisher = ChessPublisher::new(&base, "42").unwrap();
        publisher
            .publish_move(&MovePayload::from_uci("e2e4", "fen1", 1, "white"))
            .await
            .unwrap();
        let subscriber = ChessSubscriber::new(&base, "42").unwrap();
        let (receiver, task) = subscriber.subscribe_moves().await.unwrap();
        assert!(matches!(receiver.recv().await.unwrap(), ChessMessage::Move(m) if m.uci == "e2e4"));
        publisher.publish_chat("white", "hello", 1).await.unwrap();
        assert_eq!(
            node.get("game/42/chat/").await.unwrap().parents,
            vec![Version::new("root")]
        );
        publisher.publish_chat("white", "hello", 1).await.unwrap();
        let chat = node.get_updates("game/42/chat").await;
        assert_eq!(chat.len(), 2);
        assert_ne!(chat[0].version, chat[1].version);
        assert_eq!(chat[1].parents, chat[0].version);
        publisher
            .publish_move(&MovePayload::from_uci("e7e5", "fen2", 2, "black"))
            .await
            .unwrap();
        assert!(matches!(receiver.recv().await.unwrap(), ChessMessage::Move(m) if m.uci == "e7e5"));
        let history = node.get_updates("/game/42/moves").await;
        assert_eq!(history[1].parents, history[0].version);
        drop(receiver);
        task.await.unwrap();
        node.shutdown().await.unwrap();
    })
    .await
    .expect("desktop bridge stalled");
}

#[tokio::test]
async fn remote_bridge_streams_over_http3_and_preserves_versions_and_binary_payloads() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let discovery = DiscoveryConfig::mock();
        let origin = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
        discovery.add_node(origin.node_addr().await.unwrap());
        let bridge = BraidIrohNode::spawn(BraidIrohConfig {
            discovery,
            proxy_config: Some(ProxyConfig {
                listen_addr: "127.0.0.1:0".parse().unwrap(),
                default_peer: origin.node_id(),
            }),
            ..Default::default()
        })
        .await
        .unwrap();
        let client = BraidClient::new().unwrap();
        let url = format!("http://{}/game/7/moves", bridge.proxy_addr().unwrap());
        let mut subscription = client.subscribe(&url, BraidRequest::new()).await.unwrap();
        let request = BraidRequest::new()
            .with_method("PUT")
            .with_body(vec![255, 254])
            .with_version(Version::new("v1"))
            .with_parent(Version::new("root"))
            .with_content_type("application/octet-stream");
        assert_eq!(
            client.fetch(&url, request.clone()).await.unwrap().status,
            200
        );
        let update = subscription.next().await.unwrap().unwrap();
        assert_eq!(update.version, vec![Version::new("v1")]);
        assert_eq!(update.parents, vec![Version::new("root")]);
        assert_eq!(update.body.unwrap().as_ref(), &[255, 254]);
        assert_eq!(
            update.content_type.as_deref(),
            Some("application/octet-stream")
        );
        client.fetch(&url, request).await.unwrap();
        assert_eq!(origin.get_updates("game/7/moves/").await.len(), 1);
        assert!(origin.get_version("game/7/moves", "v").await.is_none());
        drop(subscription);
        origin
            .store_update(
                "game/7/moves",
                Update::snapshot(Version::new("v2"), "second")
                    .with_parents(vec![Version::new("v1")]),
            )
            .await;
        let mut resumed = client
            .subscribe(&url, BraidRequest::new().with_parent(Version::new("v1")))
            .await
            .unwrap();
        let update = resumed.next().await.unwrap().unwrap();
        assert_eq!(update.version, vec![Version::new("v2")]);
        assert_eq!(update.body_str(), Some("second"));
        drop(resumed);
        assert!(matches!(
            client
                .subscribe(
                    &url,
                    BraidRequest::new().with_parent(Version::new("missing"))
                )
                .await,
            Err(braid_http::BraidError::InvalidSubscriptionStatus(410))
        ));
        bridge.shutdown().await.unwrap();
        origin.shutdown().await.unwrap();
    })
    .await
    .expect("remote bridge stalled");
}

#[tokio::test]
async fn shutdown_flushes_history_and_corrupt_storage_fails_startup() {
    let dir = tempfile::tempdir().unwrap();
    let config = BraidIrohConfig {
        data_dir: Some(dir.path().to_owned()),
        ..Default::default()
    };
    let node = BraidIrohNode::spawn(config.clone()).await.unwrap();
    node.store_update(
        "game/1/moves/",
        Update::snapshot(Version::new("one"), "move"),
    )
    .await;
    node.shutdown().await.unwrap();
    let node = BraidIrohNode::spawn(config.clone()).await.unwrap();
    assert_eq!(
        node.get("/game/1/moves").await.unwrap().body_str(),
        Some("move")
    );
    node.shutdown().await.unwrap();
    tokio::fs::write(dir.path().join("resources.json"), "broken json")
        .await
        .unwrap();
    assert!(BraidIrohNode::spawn(config).await.is_err());
}

#[tokio::test]
async fn a_busy_bridge_port_fails_startup() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let key = iroh::SecretKey::generate();
    let result = BraidIrohNode::spawn(BraidIrohConfig {
        secret_key: Some(key.clone()),
        proxy_config: Some(ProxyConfig {
            listen_addr: listener.local_addr().unwrap(),
            default_peer: key.public(),
        }),
        ..Default::default()
    })
    .await;
    assert!(result.is_err());
}
