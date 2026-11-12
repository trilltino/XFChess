#![cfg(feature = "proxy")]
//! Transport authority over Iroh H3: default deny, read versus write grants,
//! and revocation of an already-open stream.

use anyhow::Context as _;
use braid_http::{BraidClient, BraidRequest, Update, Version};
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig, ProxyConfig};
use std::time::Duration;

struct Pair {
    origin: BraidIrohNode,
    bridge: BraidIrohNode,
    base: String,
}

async fn pair() -> anyhow::Result<Pair> {
    let discovery = DiscoveryConfig::mock();
    let origin = BraidIrohNode::spawn(BraidIrohConfig {
        discovery: discovery.clone(),
        ..Default::default()
    })
    .await?;
    discovery.add_node(origin.node_addr().await?);
    let bridge = BraidIrohNode::spawn(BraidIrohConfig {
        discovery,
        proxy_config: Some(ProxyConfig {
            listen_addr: "127.0.0.1:0".parse()?,
            default_peer: origin.node_id(),
        }),
        ..Default::default()
    })
    .await?;
    let base = format!(
        "http://{}",
        bridge.proxy_addr().context("no bridge address")?
    );
    Ok(Pair {
        origin,
        bridge,
        base,
    })
}

fn put(version: &str) -> BraidRequest {
    BraidRequest::new()
        .with_method("PUT")
        .with_body("x")
        .with_version(Version::new(version))
        .with_parent(Version::new("root"))
}

#[tokio::test]
async fn unknown_peers_are_denied_by_default() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pair = pair().await?;
        let client = BraidClient::new()?;
        let url = format!("{}/game/1/moves", pair.base);
        let denied_put = client.fetch(&url, put("v1")).await?;
        assert_eq!(denied_put.status, 403);
        assert!(pair.origin.get_updates("game/1/moves/").await.is_empty());
        let denied_get = client.subscribe(&url, BraidRequest::new()).await;
        assert!(matches!(
            denied_get,
            Err(braid_http::BraidError::SubscriptionStatus { status: 403, .. })
        ));
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("test stalled")?
}

#[tokio::test]
async fn read_grants_do_not_allow_writes() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pair = pair().await?;
        pair.origin
            .update_access(|policy| policy.allow_read(pair.bridge.node_id()));
        pair.origin
            .store_update(
                "game/2/moves",
                Update::snapshot(Version::new("v1"), "first"),
            )
            .await;
        let client = BraidClient::new()?;
        let url = format!("{}/game/2/moves", pair.base);
        let mut subscription = client.subscribe(&url, BraidRequest::new()).await?;
        let update = subscription.next().await.context("stream ended")??;
        assert_eq!(update.body_str(), Some("first"));
        assert_eq!(client.fetch(&url, put("v2")).await?.status, 403);
        assert_eq!(pair.origin.get_updates("game/2/moves").await.len(), 1);

        pair.origin
            .update_access(|policy| policy.allow_write(pair.bridge.node_id()));
        assert_eq!(client.fetch(&url, put("v3")).await?.status, 200);
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("test stalled")?
}

#[tokio::test]
async fn revoking_a_peer_closes_its_open_stream() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let pair = pair().await?;
        pair.origin
            .update_access(|policy| policy.allow_read(pair.bridge.node_id()));
        pair.origin
            .store_update(
                "game/3/moves",
                Update::snapshot(Version::new("v1"), "first"),
            )
            .await;
        let client = BraidClient::new()?;
        let url = format!("{}/game/3/moves", pair.base);
        let mut subscription = client.subscribe(&url, BraidRequest::new()).await?;
        assert!(subscription.next().await.context("stream ended")?.is_ok());

        pair.origin
            .update_access(|policy| policy.revoke(&pair.bridge.node_id()));
        pair.origin
            .store_update(
                "game/3/moves",
                Update::snapshot(Version::new("v2"), "after revocation")
                    .with_parents(vec![Version::new("v1")]),
            )
            .await;
        let next = tokio::time::timeout(Duration::from_secs(10), subscription.next())
            .await
            .context("revoked stream stayed open")?;
        assert!(
            next.is_none() || matches!(next, Some(Err(_))),
            "a revoked peer must not receive later updates"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("test stalled")?
}
