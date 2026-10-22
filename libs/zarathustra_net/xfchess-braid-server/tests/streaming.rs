use anyhow::Context as _;
use axum::body::Body;
use axum::http::Request;
use braid_http::{client::MessageParser, BraidClient, BraidRequest};
use futures::StreamExt;
use serde_json::json;
use std::future::IntoFuture;
use std::sync::Arc;
use tower::ServiceExt;
use xfchess_braid_server::{braid_router, ResourceHub};

#[tokio::test]
async fn lagging_subscribers_receive_a_snapshot_then_resume_with_correct_parents(
) -> anyhow::Result<()> {
    let hub = ResourceHub::new();
    hub.ensure_log("game/1/moves");
    let response = braid_router(hub.clone())
        .oneshot(
            Request::builder()
                .uri("/game/1/moves")
                .header("subscribe", "true")
                .body(Body::empty())
                .context("required operation value")?,
        )
        .await
        .context("required operation value")?;
    assert_eq!(response.status().as_u16(), 209);
    let mut body = response.into_body().into_data_stream();
    let mut parser = MessageParser::for_subscription();
    let first = parser
        .feed(
            &body
                .next()
                .await
                .context("required operation value")?
                .context("required operation value")?,
        )
        .context("required operation value")?;
    assert_eq!(first[0].body_text().as_deref(), Some("[]"));
    for i in 0..300 {
        hub.append("game/1/moves", json!({"move": i}));
    }
    let messages = loop {
        let messages = parser
            .feed(
                &body
                    .next()
                    .await
                    .context("required operation value")?
                    .context("required operation value")?,
            )
            .context("required operation value")?;
        if !messages.is_empty() {
            break messages;
        }
    };
    assert_eq!(
        messages[0]
            .headers
            .get("content-type")
            .context("required operation value")?,
        "application/json"
    );
    let snapshot: serde_json::Value = serde_json::from_slice(
        messages[0]
            .body
            .as_ref()
            .context("required operation value")?,
    )
    .context("required operation value")?;
    assert_eq!(
        snapshot
            .as_array()
            .context("required operation value")?
            .len(),
        300
    );
    assert_eq!(
        messages[0]
            .headers
            .get("version")
            .context("required operation value")?,
        "\"300\""
    );
    hub.append("game/1/moves", json!({"move": 300}));
    let messages = loop {
        let messages = parser
            .feed(
                &body
                    .next()
                    .await
                    .context("required operation value")?
                    .context("required operation value")?,
            )
            .context("required operation value")?;
        if !messages.is_empty() {
            break messages;
        }
    };
    assert_eq!(
        messages[0]
            .headers
            .get("parents")
            .context("required operation value")?,
        "\"300\""
    );
    assert_eq!(
        messages[0]
            .headers
            .get("version")
            .context("required operation value")?,
        "\"301\""
    );
    Ok(())
}

#[tokio::test]
async fn tournament_updates_reach_the_http_client_with_patch_metadata() -> anyhow::Result<()> {
    let hub = ResourceHub::new();
    hub.ensure_tournament(42);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("required operation value")?;
    let url = format!(
        "http://{}/tournament/42/results",
        listener.local_addr().context("required operation value")?
    );
    let server = tokio::spawn(axum::serve(listener, braid_router(hub.clone())).into_future());
    let client = BraidClient::new().context("required operation value")?;
    let mut subscription = client
        .subscribe(&url, BraidRequest::new())
        .await
        .context("required operation value")?;
    let snapshot = subscription
        .next()
        .await
        .context("required operation value")?
        .context("required operation value")?;
    assert_eq!(snapshot.body_str(), Some("[]"));
    hub.append(
        "tournament/42/results",
        json!({"round": 1, "board": 1, "result": "Draw"}),
    );
    let update = subscription
        .next()
        .await
        .context("required operation value")?
        .context("required operation value")?;
    assert_eq!(
        update.content_type.as_deref(),
        Some("application/json-patch+json")
    );
    assert_eq!(update.parents[0].to_string(), "0");
    assert_eq!(update.url.as_deref(), Some("/tournament/42/results"));
    drop(subscription);
    server.abort();
    Ok(())
}

#[tokio::test]
async fn initializing_resources_concurrently_preserves_existing_state_and_subscribers(
) -> anyhow::Result<()> {
    let hub = ResourceHub::new();
    hub.ensure_tournament(1);
    hub.append("tournament/1/results", json!(1));
    let (snapshot, mut rx) = hub
        .subscribe("tournament/1/results")
        .await
        .context("required operation value")?;
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let hub = hub.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    hub.ensure_tournament(1);
                }
            })
        })
        .collect();
    for worker in workers {
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("resource initialization thread panicked"))?;
    }
    hub.append("tournament/1/results", json!(2));
    assert_eq!(snapshot.body, json!([1]));
    assert_eq!(
        rx.recv().await.context("required operation value")?.version,
        2
    );
    assert_eq!(
        hub.current_json("tournament/1/results")
            .await
            .context("required operation value")?,
        json!([1, 2])
    );
    Ok(())
}

#[tokio::test]
async fn gossip_callbacks_can_modify_the_hub_without_holding_its_resource_lock(
) -> anyhow::Result<()> {
    let hub = ResourceHub::new();
    hub.register_doc("state", json!({}));
    let callback_hub = hub.clone();
    hub.set_gossip_sink(Arc::new(move |_, _| {
        callback_hub.ensure_log("events");
    }));
    let worker_hub = hub.clone();
    let worker =
        tokio::task::spawn_blocking(move || worker_hub.replace("state", json!({"ready": true})));
    tokio::time::timeout(std::time::Duration::from_secs(2), worker)
        .await
        .context("required operation value")?
        .context("required operation value")?;
    hub.set_gossip_sink(Arc::new(|_, _| {}));
    assert!(hub.has("events"));
    Ok(())
}
