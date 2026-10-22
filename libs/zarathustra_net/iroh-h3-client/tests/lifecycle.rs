use anyhow::Context as _;
#[path = "../../iroh-h3/tests/support/mod.rs"]
mod support;

use axum::{Router, body::Body, routing::get};
use bytes::Bytes;
use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh_h3_axum::IrohAxum;
use iroh_h3_client::IrohH3Client;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Notify, oneshot};

const ALPN: &[u8] = b"iroh+h3";

#[derive(Debug)]
struct ObservedServer {
    server: IrohAxum,
    closed: Arc<Notify>,
}

impl ProtocolHandler for ObservedServer {
    async fn accept(&self, connection: iroh::endpoint::Connection) -> Result<(), AcceptError> {
        let observed = connection.clone();
        let closed = self.closed.clone();
        tokio::spawn(async move {
            observed.closed().await;
            closed.notify_one();
        });
        self.server.accept(connection).await
    }
}

#[tokio::test]
async fn response_outlives_client_and_then_releases_its_connection() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let discovery = support::MockDiscoveryMap::new();
        let origin = discovery.spawn_endpoint().await?;
        let endpoint = discovery.spawn_endpoint().await?;
        let (ready, receive) = oneshot::channel::<()>();
        let receive = Arc::new(Mutex::new(Some(receive)));
        let app = Router::new().route(
            "/",
            get(move || {
                let receive = receive.clone();
                async move {
                    let receive = receive
                        .lock()
                        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
                        .take()
                        .ok_or(axum::http::StatusCode::CONFLICT)?;
                    Ok::<_, axum::http::StatusCode>(Body::from_stream(futures::stream::once(
                        async move {
                            receive.await.map_err(std::io::Error::other)?;
                            Ok::<_, std::io::Error>(Bytes::from_static(b"response survived"))
                        },
                    )))
                }
            }),
        );
        let closed = Arc::new(Notify::new());
        let router = iroh::protocol::Router::builder(origin.clone())
            .accept(
                ALPN,
                ObservedServer {
                    server: IrohAxum::new(app),
                    closed: closed.clone(),
                },
            )
            .spawn();
        let client = IrohH3Client::new(endpoint.clone(), ALPN.to_vec());
        let response = client
            .get(format!("iroh+h3://{}/", origin.id()))
            .send()
            .await
            .context("required operation value")?;
        drop(client);
        ready
            .send(())
            .map_err(|_| anyhow::anyhow!("response body receiver dropped"))?;
        assert_eq!(
            response.bytes().await.context("required operation value")?,
            b"response survived".as_slice()
        );
        closed.notified().await;
        router
            .shutdown()
            .await
            .context("required operation value")?;
        endpoint.close().await;
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("HTTP/3 client retained an abandoned connection")??;
    Ok(())
}
