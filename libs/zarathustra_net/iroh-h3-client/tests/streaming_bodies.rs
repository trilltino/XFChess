use anyhow::Context as _;
use std::convert::Infallible;

use axum::{
    Router,
    body::Body,
    response::IntoResponse,
    routing::{get, post},
};
use bytes::Bytes;
#[path = "../../iroh-h3/tests/support/mod.rs"]
mod support;
use futures::StreamExt;
use http_body::Frame;
use http_body_util::{StreamBody, combinators::BoxBody};
use iroh_h3_axum::IrohAxum;
use iroh_h3_client::IrohH3Client;
use support::MockDiscoveryMap;

const ALPN: &[u8] = b"iroh+h3";

#[tokio::test]
async fn response_body_errors_are_not_reported_as_successful_eof() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let server = discovery.spawn_endpoint().await?;
    let endpoint = discovery.spawn_endpoint().await?;
    let app = Router::new().route(
        "/",
        get(|| async {
            Body::from_stream(futures::stream::iter([
                Ok(Bytes::from_static(b"partial")),
                Err(std::io::Error::other("failed body")),
            ]))
        }),
    );
    let router = iroh::protocol::Router::builder(server.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();
    let client = IrohH3Client::new(endpoint.clone(), ALPN.into());
    let result = async {
        client
            .get(format!("https://{}/", server.id()))
            .send()
            .await?
            .bytes()
            .await
    }
    .await;
    assert!(result.is_err());
    endpoint.close().await;
    router
        .shutdown()
        .await
        .context("required operation value")?;
    Ok(())
}

#[tokio::test]
async fn streaming_response() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let endpoint_1 = discovery.spawn_endpoint().await?;
    let endpoint_2 = discovery.spawn_endpoint().await?;

    async fn streaming_ping() -> impl IntoResponse {
        let stream = futures::stream::repeat(Ok::<Bytes, Infallible>(Bytes::from_static(b"Pong!")));
        Body::from_stream(stream.take(10))
    }

    let app = Router::new().route("/streaming-ping", get(streaming_ping));
    let _router = iroh::protocol::Router::builder(endpoint_1.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();

    let client = IrohH3Client::new(endpoint_2, ALPN.into());
    let uri = format!("iroh+h3://{}/streaming-ping", endpoint_1.id());
    let response = client
        .get(&uri)
        .send()
        .await
        .context("required operation value")?;

    let mut stream = response.bytes_stream();
    let mut count = 0usize;
    while let Some(chunk) = stream
        .next()
        .await
        .transpose()
        .context("required operation value")?
    {
        assert_eq!(chunk, b"Pong!"[..]);
        count += 1;
    }
    assert_eq!(count, 10);
    Ok(())
}

#[tokio::test]
async fn streaming_request_body() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let endpoint_1 = discovery.spawn_endpoint().await?;
    let endpoint_2 = discovery.spawn_endpoint().await?;

    const PING: &str = "Ping!";
    const PING_COUNT: usize = 5;
    const PONG: &str = "Pong!";
    const PONG_COUNT: usize = 7;

    async fn streaming_ping(body: Body) -> Result<Body, axum::http::StatusCode> {
        let mut body_stream = body.into_data_stream();
        let mut counter = 0usize;
        while let Some(chunk) = body_stream
            .next()
            .await
            .transpose()
            .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?
        {
            assert_eq!(chunk, PING.as_bytes());
            counter += 1;
        }
        assert_eq!(counter, PING_COUNT);

        let pong_bytes = Bytes::from_static(PONG.as_bytes());
        let ok = Ok::<Bytes, Infallible>(pong_bytes);
        Ok(Body::from_stream(
            futures::stream::repeat(ok).take(PONG_COUNT),
        ))
    }

    let app = Router::new().route("/streaming-ping", post(streaming_ping));
    let _router = iroh::protocol::Router::builder(endpoint_1.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();

    let client = IrohH3Client::new(endpoint_2, ALPN.into());
    let uri = format!("iroh+h3://{}/streaming-ping", endpoint_1.id());

    let ping_bytes = Bytes::from_static(PING.as_bytes());
    let frame = move || Frame::data(ping_bytes.clone());
    let stream = futures::stream::repeat_with(move || Ok::<_, Infallible>(frame()));
    let body = BoxBody::new(StreamBody::new(stream.take(PING_COUNT))).into();

    let response = client
        .post(uri)
        .body(body)
        .context("required operation value")?
        .send()
        .await
        .context("required operation value")?;
    let mut resp_stream = response.bytes_stream();
    let mut count = 0usize;
    while let Some(chunk) = resp_stream
        .next()
        .await
        .transpose()
        .context("required operation value")?
    {
        assert_eq!(chunk, PONG.as_bytes());
        count += 1;
    }
    assert_eq!(count, PONG_COUNT);
    Ok(())
}
