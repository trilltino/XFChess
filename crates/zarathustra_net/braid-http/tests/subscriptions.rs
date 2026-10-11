use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use braid_http::{client::ClientConfig, BraidClient, BraidError, BraidRequest};
use bytes::Bytes;
use std::{
    future::IntoFuture,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

async fn server(router: Router) -> (String, tokio::task::JoinHandle<std::io::Result<()>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(axum::serve(listener, router).into_future()),
    )
}

#[tokio::test]
async fn rejects_error_pages_instead_of_opening_a_subscription() {
    let (url, server) =
        server(Router::new().route("/", get(|| async { StatusCode::UNAUTHORIZED }))).await;
    let result = BraidClient::new()
        .unwrap()
        .subscribe(&url, BraidRequest::new())
        .await;
    assert!(matches!(
        result,
        Err(BraidError::InvalidSubscriptionStatus(401))
    ));
    server.abort();
}

#[tokio::test]
async fn wire_heartbeats_keep_a_quiet_stream_alive_and_dropping_it_cancels_the_transport() {
    let dropped = Arc::new(AtomicBool::new(false));
    let observed = dropped.clone();
    let router = Router::new().route("/", get(move || {
        let dropped = dropped.clone();
        async move {
            let stream = async_stream::stream! {
                let _guard = Dropped(dropped);
                let mut ticker = tokio::time::interval(Duration::from_millis(25));
                loop { ticker.tick().await; yield Ok::<Bytes, std::io::Error>(Bytes::from_static(b"\r\n")); }
            };
            Response::builder().status(209).header("heartbeats", "0.025").body(Body::from_stream(stream)).unwrap().into_response()
        }
    }));
    let (url, server) = server(router).await;
    let client = BraidClient::with_config(ClientConfig {
        request_timeout_ms: 250,
        ..Default::default()
    })
    .unwrap();
    let mut subscription = client.subscribe(&url, BraidRequest::new()).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(3500), subscription.next())
            .await
            .is_err()
    );
    drop(subscription);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !observed.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("abandoned transport remained open");
    server.abort();
}

#[tokio::test]
async fn a_wire_idle_timeout_reports_one_error_then_ends() {
    let router = Router::new().route(
        "/",
        get(|| async {
            let stream = futures::stream::once(async {
                Ok::<Bytes, std::io::Error>(Bytes::from_static(b"\r\n"))
            })
            .chain(futures::stream::pending());
            Response::builder()
                .status(209)
                .header("heartbeats", "0.01")
                .body(Body::from_stream(stream))
                .unwrap()
        }),
    );
    let (url, server) = server(router).await;
    let mut subscription = BraidClient::new()
        .unwrap()
        .subscribe(&url, BraidRequest::new())
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), subscription.next())
            .await
            .unwrap(),
        Some(Err(BraidError::Timeout))
    ));
    assert!(subscription.next().await.is_none());
    server.abort();
}

use futures::StreamExt;
