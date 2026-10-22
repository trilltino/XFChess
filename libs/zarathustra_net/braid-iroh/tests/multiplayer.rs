#![cfg(feature = "proxy")]

use anyhow::Context as _;
use braid_chess::{ChessMessage, ChessPublisher, ChessSubscriber, MovePayload};
use braid_http::{BraidClient, BraidRequest, Update, Version};
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig, ProxyConfig};
use futures::StreamExt;
use std::time::Duration;

#[tokio::test]
async fn game_updates_cross_gossip_topics_between_two_local_peers() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let discovery = DiscoveryConfig::mock();
        let a = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .context("required operation value")?;
        let b = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .context("required operation value")?;
        discovery.add_node(a.node_addr().await.context("required operation value")?);
        discovery.add_node(b.node_addr().await.context("required operation value")?);
        let mut a_events = a
            .subscribe("game/9/moves/", vec![])
            .await
            .context("required operation value")?;
        let mut b_events = b
            .subscribe("/game/9/moves", vec![a.node_id()])
            .await
            .context("required operation value")?;
        while !matches!(
            a_events
                .next()
                .await
                .context("required operation value")?
                .context("required operation value")?,
            iroh_gossip::api::Event::NeighborUp(_)
        ) {}
        let update = Update::snapshot(Version::new("move-1"), "e2e4");
        a.put("game/9/moves", update.clone())
            .await
            .context("required operation value")?;
        loop {
            if let iroh_gossip::api::Event::Received(message) = b_events
                .next()
                .await
                .context("required operation value")?
                .context("required operation value")?
            {
                let received: Update =
                    serde_json::from_slice(&message.content).context("required operation value")?;
                assert_eq!(received.version, update.version);
                assert_eq!(received.body_str(), Some("e2e4"));
                break;
            }
        }
        let raw = Update::snapshot(Version::new("move-2"), vec![0x02, 0, 255, 128]);
        a.put_raw("game/9/moves", raw.clone())
            .await
            .context("required operation value")?;
        loop {
            if let iroh_gossip::api::Event::Received(message) = b_events
                .next()
                .await
                .context("required operation value")?
                .context("required operation value")?
            {
                assert_eq!(
                    message.content,
                    raw.body.clone().context("required operation value")?
                );
                break;
            }
        }
        assert_eq!(
            a.get("game/9/moves")
                .await
                .context("required operation value")?
                .body,
            raw.body
        );
        assert_eq!(a.get_updates("game/9/moves").await.len(), 2);
        a.shutdown().await.context("required operation value")?;
        b.shutdown().await.context("required operation value")?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("gossip game update stalled")??;
    Ok(())
}

#[tokio::test]
async fn desktop_bridge_delivers_history_live_moves_and_independent_chat_parents(
) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let key = iroh::SecretKey::generate();
        let node = BraidIrohNode::spawn(BraidIrohConfig {
            secret_key: Some(key.clone()),
            proxy_config: Some(ProxyConfig {
                listen_addr: "127.0.0.1:0".parse().context("required operation value")?,
                default_peer: key.public(),
            }),
            ..Default::default()
        })
        .await
        .context("required operation value")?;
        let base = format!("http://{}", node.proxy_addr().context("required operation value")?);
        let mut publisher = ChessPublisher::new(&base, "42").context("required operation value")?;
        publisher
            .publish_move(&MovePayload::from_uci("e2e4", "fen1", 1, "white"))
            .await
            .context("required operation value")?;
        let subscriber = ChessSubscriber::new(&base, "42").context("required operation value")?;
        let (receiver, task) = subscriber.subscribe_moves().await.context("required operation value")?;
        assert!(matches!(receiver.recv().await.context("required operation value")?, ChessMessage::Move(m) if m.uci == "e2e4"));
        publisher.publish_chat("white", "hello", 1).await.context("required operation value")?;
        assert_eq!(
            node.get("game/42/chat/").await.context("required operation value")?.parents,
            vec![Version::new("root")]
        );
        publisher.publish_chat("white", "hello", 1).await.context("required operation value")?;
        let chat = node.get_updates("game/42/chat").await;
        assert_eq!(chat.len(), 2);
        assert_ne!(chat[0].version, chat[1].version);
        assert_eq!(chat[1].parents, chat[0].version);
        publisher
            .publish_move(&MovePayload::from_uci("e7e5", "fen2", 2, "black"))
            .await
            .context("required operation value")?;
        assert!(matches!(receiver.recv().await.context("required operation value")?, ChessMessage::Move(m) if m.uci == "e7e5"));
        let history = node.get_updates("/game/42/moves").await;
        assert_eq!(history[1].parents, history[0].version);
        drop(receiver);
        task.await.context("required operation value")?;
        node.shutdown().await.context("required operation value")?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("desktop bridge stalled")??;
    Ok(())
}

#[tokio::test]
async fn remote_bridge_streams_over_http3_and_preserves_versions_and_binary_payloads(
) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let discovery = DiscoveryConfig::mock();
        let origin = BraidIrohNode::spawn(BraidIrohConfig {
            discovery: discovery.clone(),
            ..Default::default()
        })
        .await
        .context("required operation value")?;
        discovery.add_node(
            origin
                .node_addr()
                .await
                .context("required operation value")?,
        );
        let bridge = BraidIrohNode::spawn(BraidIrohConfig {
            discovery,
            proxy_config: Some(ProxyConfig {
                listen_addr: "127.0.0.1:0".parse().context("required operation value")?,
                default_peer: origin.node_id(),
            }),
            ..Default::default()
        })
        .await
        .context("required operation value")?;
        let client = BraidClient::new().context("required operation value")?;
        let url = format!(
            "http://{}/game/7/moves",
            bridge.proxy_addr().context("required operation value")?
        );
        let mut subscription = client
            .subscribe(&url, BraidRequest::new())
            .await
            .context("required operation value")?;
        let request = BraidRequest::new()
            .with_method("PUT")
            .with_body(vec![255, 254])
            .with_version(Version::new("v1"))
            .with_parent(Version::new("root"))
            .with_content_type("application/octet-stream");
        assert_eq!(
            client
                .fetch(&url, request.clone())
                .await
                .context("required operation value")?
                .status,
            200
        );
        let update = subscription
            .next()
            .await
            .context("required operation value")?
            .context("required operation value")?;
        assert_eq!(update.version, vec![Version::new("v1")]);
        assert_eq!(update.parents, vec![Version::new("root")]);
        assert_eq!(
            update.body.context("required operation value")?.as_ref(),
            &[255, 254]
        );
        assert_eq!(
            update.content_type.as_deref(),
            Some("application/octet-stream")
        );
        client
            .fetch(&url, request)
            .await
            .context("required operation value")?;
        assert_eq!(origin.get_updates("game/7/moves/").await.len(), 1);
        assert!(origin.get_version("game/7/moves", "v").await.is_none());
        drop(subscription);
        let raw = Update::snapshot(Version::new("raw-v1"), vec![0x02, 0, 255, 128]);
        origin
            .put_raw("game/7/raw", raw.clone())
            .await
            .context("required operation value")?;
        let raw_url = format!(
            "http://{}/game/7/raw",
            bridge.proxy_addr().context("required operation value")?
        );
        let mut raw_subscription = client
            .subscribe(&raw_url, BraidRequest::new())
            .await
            .context("required operation value")?;
        let from_bridge = raw_subscription
            .next()
            .await
            .context("required operation value")?
            .context("required operation value")?;
        assert_eq!(from_bridge.version, raw.version);
        assert_eq!(from_bridge.body, raw.body);
        drop(raw_subscription);
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
            .context("required operation value")?;
        let update = resumed
            .next()
            .await
            .context("required operation value")?
            .context("required operation value")?;
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
        bridge
            .shutdown()
            .await
            .context("required operation value")?;
        origin
            .shutdown()
            .await
            .context("required operation value")?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("remote bridge stalled")??;
    Ok(())
}

#[tokio::test]
async fn shutdown_flushes_history_and_corrupt_storage_fails_startup() -> anyhow::Result<()> {
    let dir = tempfile::tempdir().context("required operation value")?;
    let config = BraidIrohConfig {
        data_dir: Some(dir.path().to_owned()),
        ..Default::default()
    };
    let node = BraidIrohNode::spawn(config.clone())
        .await
        .context("required operation value")?;
    node.store_update(
        "game/1/moves/",
        Update::snapshot(Version::new("one"), "move"),
    )
    .await;
    node.shutdown().await.context("required operation value")?;
    let node = BraidIrohNode::spawn(config.clone())
        .await
        .context("required operation value")?;
    assert_eq!(
        node.get("/game/1/moves")
            .await
            .context("required operation value")?
            .body_str(),
        Some("move")
    );
    node.shutdown().await.context("required operation value")?;
    tokio::fs::write(dir.path().join("resources.json"), "broken json")
        .await
        .context("required operation value")?;
    assert!(BraidIrohNode::spawn(config).await.is_err());
    Ok(())
}

#[tokio::test]
async fn a_busy_bridge_port_fails_startup() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("required operation value")?;
    let key = iroh::SecretKey::generate();
    let result = BraidIrohNode::spawn(BraidIrohConfig {
        secret_key: Some(key.clone()),
        proxy_config: Some(ProxyConfig {
            listen_addr: listener.local_addr().context("required operation value")?,
            default_peer: key.public(),
        }),
        ..Default::default()
    })
    .await;
    assert!(result.is_err());
    Ok(())
}
