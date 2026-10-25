//! XFChess regression coverage for subscriptions that must survive a flaky
//! network: resume from the acknowledged frontier, bounded retries, fatal
//! protocol errors, and cancellation on drop.

use anyhow::Context as _;
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use braid_http::{client::RetryConfig, BraidClient, BraidError, BraidRequest, Version};
use bytes::Bytes;
use futures::StreamExt;
use std::{
    future::IntoFuture,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

fn server_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

async fn server(
    router: Router,
) -> anyhow::Result<(String, tokio::task::JoinHandle<std::io::Result<()>>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    Ok((
        url,
        tokio::spawn(axum::serve(listener, router).into_future()),
    ))
}

fn retry(limit: u32) -> BraidRequest {
    BraidRequest::new().with_retry(
        RetryConfig::default()
            .with_max_retries(limit)
            .with_initial_backoff(Duration::from_millis(10))
            .with_max_backoff(Duration::from_millis(20)),
    )
}

#[tokio::test]
async fn resumes_from_complete_updates_after_a_truncated_connection() -> anyhow::Result<()> {
    let parents = Arc::new(Mutex::new(Vec::new()));
    let received = parents.clone();
    let router = Router::new().route(
        "/",
        get(move |headers: HeaderMap| {
            let received = received.clone();
            async move {
                let mut parents = received.lock().map_err(server_error)?;
                parents.push(
                    headers
                        .get("parents")
                        .map(|value| value.to_str().map(str::to_owned))
                        .transpose()
                        .map_err(server_error)?,
                );
                let first = parents.len() == 1;
                drop(parents);
                let wire: &'static [u8] = if first {
                    b"Version: \"v1\"\r\nParents: \"root\"\r\nContent-Length: 1\r\n\r\na\r\nVersion: \"v2\"\r\nParents: \"v1\"\r\nContent-Length: 2\r\n\r\nb"
                } else {
                    b"Version: \"v2\"\r\nParents: \"v1\"\r\nContent-Length: 2\r\n\r\nbb\r\n"
                };
                let stream = futures::stream::once(async move {
                    Ok::<_, std::io::Error>(Bytes::from_static(wire))
                });
                let body = if first {
                    Body::from_stream(stream)
                } else {
                    Body::from_stream(stream.chain(futures::stream::pending()))
                };
                Response::builder()
                    .status(209)
                    .body(body)
                    .map_err(server_error)
            }
        }),
    );
    let (url, server) = server(router).await?;
    let mut subscription = BraidClient::new()?
        .subscribe_reliable(&url, retry(2))
        .await?;
    for (version, body) in [("v1", "a"), ("v2", "bb")] {
        let update = tokio::time::timeout(Duration::from_secs(3), subscription.next())
            .await?
            .context("stream ended")??;
        assert_eq!(update.version, vec![Version::new(version)]);
        assert_eq!(update.body_str(), Some(body));
        subscription.acknowledge(&update).await?;
    }
    assert_eq!(
        *parents
            .lock()
            .map_err(|error| anyhow::anyhow!("test state lock poisoned: {error}"))?,
        vec![None, Some("\"v1\"".to_owned())],
        "the truncated v2 is requested again, resuming from the acknowledged v1"
    );
    drop(subscription);
    server.abort();
    Ok(())
}

#[tokio::test]
async fn temporary_handshake_failure_retries_but_access_denial_does_not() -> anyhow::Result<()> {
    for status in [
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::UNAUTHORIZED,
    ] {
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let router = Router::new().route(
            "/",
            get(move || {
                let observed = observed.clone();
                async move {
                    if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                        Response::builder()
                            .status(status)
                            .header("Retry-After", "0")
                            .body(Body::empty())
                            .map_err(server_error)
                    } else {
                        let stream = futures::stream::pending::<Result<Bytes, std::io::Error>>();
                        Response::builder()
                            .status(209)
                            .body(Body::from_stream(stream))
                            .map_err(server_error)
                    }
                }
            }),
        );
        let (url, server) = server(router).await?;
        let mut subscription = BraidClient::new()?
            .subscribe_reliable(&url, retry(2))
            .await?;
        if status == StatusCode::UNAUTHORIZED {
            let error = tokio::time::timeout(Duration::from_secs(3), subscription.next())
                .await?
                .context("stream ended")?
                .err()
                .context("access denial must surface as an error")?;
            assert!(matches!(
                error,
                BraidError::SubscriptionStatus { status: 401, .. }
            ));
            assert_eq!(attempts.load(Ordering::SeqCst), 1);
        } else {
            // The retry reaches the healthy stream, which then stays quiet.
            assert!(
                tokio::time::timeout(Duration::from_millis(500), subscription.next())
                    .await
                    .is_err()
            );
            assert_eq!(attempts.load(Ordering::SeqCst), 2);
        }
        server.abort();
    }
    Ok(())
}

#[tokio::test]
async fn empty_connections_exhaust_the_budget_and_malformed_updates_are_fatal() -> anyhow::Result<()>
{
    for malformed in [false, true] {
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let router = Router::new().route(
            "/",
            get(move || {
                let observed = observed.clone();
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    let wire = if malformed {
                        "Content-Length: nope\r\n\r\n"
                    } else {
                        ""
                    };
                    Response::builder()
                        .status(209)
                        .body(Body::from(wire))
                        .map_err(server_error)
                }
            }),
        );
        let (url, server) = server(router).await?;
        let mut subscription = BraidClient::new()?
            .subscribe_reliable(&url, retry(2))
            .await?;
        let error = tokio::time::timeout(Duration::from_secs(3), subscription.next())
            .await?
            .context("stream ended")?
            .err()
            .context("an error is expected")?;
        if malformed {
            assert!(matches!(
                error,
                BraidError::Protocol(_) | BraidError::HeaderParse(_)
            ));
            assert_eq!(attempts.load(Ordering::SeqCst), 1);
        } else {
            assert!(matches!(error, BraidError::SubscriptionClosed));
            assert_eq!(attempts.load(Ordering::SeqCst), 3);
        }
        assert!(subscription.next().await.is_none());
        server.abort();
    }
    Ok(())
}

#[tokio::test]
async fn dropping_a_subscription_cancels_pending_reconnects() -> anyhow::Result<()> {
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let router = Router::new().route(
        "/",
        get(move || {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                Response::builder()
                    .status(209)
                    .body(Body::empty())
                    .map_err(server_error)
            }
        }),
    );
    let (url, server) = server(router).await?;
    let request = BraidRequest::new().with_retry(
        RetryConfig::default()
            .with_initial_backoff(Duration::from_millis(100))
            .with_max_backoff(Duration::from_millis(100)),
    );
    let subscription = BraidClient::new()?
        .subscribe_reliable(&url, request)
        .await?;
    tokio::time::sleep(Duration::from_millis(30)).await;
    drop(subscription);
    let settled = attempts.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(attempts.load(Ordering::SeqCst), settled);
    server.abort();
    Ok(())
}

#[tokio::test]
async fn http2_subscriptions_share_a_connection() -> anyhow::Result<()> {
    use axum::extract::ConnectInfo;
    use std::{collections::HashSet, net::SocketAddr};
    let peers = Arc::new(Mutex::new(HashSet::new()));
    let observed = peers.clone();
    let router = Router::new().route(
        "/",
        get(move |ConnectInfo(peer): ConnectInfo<SocketAddr>| {
            let observed = observed.clone();
            async move {
                observed.lock().map_err(server_error)?.insert(peer);
                let stream = futures::stream::once(async {
                    Ok::<_, std::io::Error>(Bytes::from_static(
                        b"Version: \"one\"\r\nContent-Length: 1\r\n\r\nx",
                    ))
                })
                .chain(futures::stream::pending());
                Response::builder()
                    .status(209)
                    .body(Body::from_stream(stream))
                    .map_err(server_error)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .into_future(),
    );
    let client =
        BraidClient::with_client(reqwest::Client::builder().http2_prior_knowledge().build()?)?;
    let mut subscriptions = vec![client.subscribe(&url, BraidRequest::new()).await?];
    let results =
        futures::future::join_all((0..8).map(|_| client.subscribe(&url, BraidRequest::new())))
            .await;
    for result in results {
        subscriptions.push(result?);
    }
    for subscription in &mut subscriptions {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), subscription.next())
                .await?
                .context("stream ended")??
                .body_str(),
            Some("x")
        );
    }
    assert_eq!(
        peers
            .lock()
            .map_err(|error| anyhow::anyhow!("test state lock poisoned: {error}"))?
            .len(),
        1
    );
    drop(subscriptions);
    server.abort();
    Ok(())
}
