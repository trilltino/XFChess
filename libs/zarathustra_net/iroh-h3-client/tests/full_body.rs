use anyhow::Context as _;
use axum::{Router, body::Body, response::IntoResponse, routing::post};
use bytes::Bytes;
#[path = "../../iroh-h3/tests/support/mod.rs"]
mod support;
use http_body_util::BodyExt as _;
use iroh_h3_axum::IrohAxum;
use iroh_h3_client::IrohH3Client;
use support::MockDiscoveryMap;

const ALPN: &[u8] = b"iroh+h3";

#[tokio::test]
async fn full_body_helpers() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let endpoint_1 = discovery.spawn_endpoint().await?;
    let endpoint_2 = discovery.spawn_endpoint().await?;

    async fn echo_full(body: Body) -> Result<impl IntoResponse, axum::http::StatusCode> {
        let body = body
            .collect()
            .await
            .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
        Ok(body.to_bytes())
    }

    let app = Router::new().route("/echo", post(echo_full));
    let _router = iroh::protocol::Router::builder(endpoint_1.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();

    let client = IrohH3Client::new(endpoint_2, ALPN.into());
    let uri = format!("iroh+h3://{}/echo", endpoint_1.id());

    let payload = Bytes::from_static(b"hello-bytes");
    let request = client
        .post(&uri)
        .bytes(payload.clone())
        .context("required operation value")?;
    let response = request.send().await.context("required operation value")?;

    let got = response.bytes().await.context("required operation value")?;
    assert_eq!(got, payload);
    Ok(())
}
