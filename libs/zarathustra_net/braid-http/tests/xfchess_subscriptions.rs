use anyhow::Context as _;
use axum::{body::Body, http::StatusCode, response::Response, routing::get, Router};
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

async fn server(
    router: Router,
) -> anyhow::Result<(String, tokio::task::JoinHandle<std::io::Result<()>>)> {
    Ok({
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("required operation value")?;
        let url = format!(
            "http://{}",
            listener.local_addr().context("required operation value")?
        );
        (
            url,
            tokio::spawn(axum::serve(listener, router).into_future()),
        )
    })
}

#[tokio::test]
async fn rejects_error_pages_instead_of_opening_a_subscription() -> anyhow::Result<()> {
    let (url, server) =
        server(Router::new().route("/", get(|| async { StatusCode::UNAUTHORIZED }))).await?;
    let result = BraidClient::new()
        .context("required operation value")?
        .subscribe(&url, BraidRequest::new())
        .await;
    assert!(matches!(
        result,
        Err(BraidError::SubscriptionStatus { status: 401, .. })
    ));
    server.abort();
    Ok(())
}

#[tokio::test]
async fn wire_heartbeats_keep_a_quiet_stream_alive_and_dropping_it_cancels_the_transport(
) -> anyhow::Result<()> {
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
            Response::builder().status(209).header("heartbeats", "0.025").body(Body::from_stream(stream))
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
        }
    }));
    let (url, server) = server(router).await?;
    let client = BraidClient::with_config(ClientConfig {
        request_timeout_ms: 250,
        ..Default::default()
    })
    .context("required operation value")?;
    let mut subscription = client
        .subscribe(&url, BraidRequest::new())
        .await
        .context("required operation value")?;
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
    .context("abandoned transport remained open")?;
    server.abort();
    Ok(())
}

#[tokio::test]
async fn a_wire_idle_timeout_reports_one_error_then_ends() -> anyhow::Result<()> {
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
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
        }),
    );
    let (url, server) = server(router).await?;
    let mut subscription = BraidClient::new()
        .context("required operation value")?
        .subscribe(&url, BraidRequest::new())
        .await
        .context("required operation value")?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), subscription.next())
            .await
            .context("required operation value")?,
        Some(Err(BraidError::Timeout))
    ));
    assert!(subscription.next().await.is_none());
    server.abort();
    Ok(())
}

use futures::StreamExt;
