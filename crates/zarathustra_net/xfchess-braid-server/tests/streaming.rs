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
async fn lagging_subscribers_receive_a_snapshot_then_resume_with_correct_parents() {
    let hub = ResourceHub::new();
    hub.ensure_log("game/1/moves");
    let response = braid_router(hub.clone())
        .oneshot(
            Request::builder()
                .uri("/game/1/moves")
                .header("subscribe", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 209);
    let mut body = response.into_body().into_data_stream();
    let mut parser = MessageParser::for_subscription();
    let first = parser.feed(&body.next().await.unwrap().unwrap()).unwrap();
    assert_eq!(first[0].body_text().as_deref(), Some("[]"));
    for i in 0..300 {
        hub.append("game/1/moves", json!({"move": i}));
    }
    let messages = loop {
        let messages = parser.feed(&body.next().await.unwrap().unwrap()).unwrap();
        if !messages.is_empty() {
            break messages;
        }
    };
    assert_eq!(
        messages[0].headers.get("content-type").unwrap(),
        "application/json"
    );
    let snapshot: serde_json::Value =
        serde_json::from_slice(messages[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(snapshot.as_array().unwrap().len(), 300);
    assert_eq!(messages[0].headers.get("version").unwrap(), "\"300\"");
    hub.append("game/1/moves", json!({"move": 300}));
    let messages = loop {
        let messages = parser.feed(&body.next().await.unwrap().unwrap()).unwrap();
        if !messages.is_empty() {
            break messages;
        }
    };
    assert_eq!(messages[0].headers.get("parents").unwrap(), "\"300\"");
    assert_eq!(messages[0].headers.get("version").unwrap(), "\"301\"");
}

#[tokio::test]
async fn tournament_updates_reach_the_http_client_with_patch_metadata() {
    let hub = ResourceHub::new();
    hub.ensure_tournament(42);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/tournament/42/results",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(axum::serve(listener, braid_router(hub.clone())).into_future());
    let client = BraidClient::new().unwrap();
    let mut subscription = client.subscribe(&url, BraidRequest::new()).await.unwrap();
    let snapshot = subscription.next().await.unwrap().unwrap();
    assert_eq!(snapshot.body_str(), Some("[]"));
    hub.append(
        "tournament/42/results",
        json!({"round": 1, "board": 1, "result": "Draw"}),
    );
    let update = subscription.next().await.unwrap().unwrap();
    assert_eq!(
        update.content_type.as_deref(),
        Some("application/json-patch+json")
    );
    assert_eq!(update.parents[0].to_string(), "0");
    assert_eq!(update.url.as_deref(), Some("/tournament/42/results"));
    drop(subscription);
    server.abort();
}

#[tokio::test]
async fn initializing_resources_concurrently_preserves_existing_state_and_subscribers() {
    let hub = ResourceHub::new();
    hub.ensure_tournament(1);
    hub.append("tournament/1/results", json!(1));
    let (snapshot, mut rx) = hub.subscribe("tournament/1/results").await.unwrap();
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
        worker.join().unwrap();
    }
    hub.append("tournament/1/results", json!(2));
    assert_eq!(snapshot.body, json!([1]));
    assert_eq!(rx.recv().await.unwrap().version, 2);
    assert_eq!(
        hub.current_json("tournament/1/results").await.unwrap(),
        json!([1, 2])
    );
}

#[tokio::test]
async fn gossip_callbacks_can_modify_the_hub_without_holding_its_resource_lock() {
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
        .unwrap()
        .unwrap();
    hub.set_gossip_sink(Arc::new(|_, _| {}));
    assert!(hub.has("events"));
}
