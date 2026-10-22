use anyhow::Context as _;
use axum::{Router, extract::State, response::IntoResponse, routing::get};
#[path = "../../iroh-h3/tests/support/mod.rs"]
mod support;
use iroh::EndpointId;
use iroh_h3_axum::{IrohAxum, RemoteId};
use iroh_h3_client::IrohH3Client;
use support::MockDiscoveryMap;

const ALPN: &[u8] = b"iroh+h3";

#[tokio::test]
async fn remote_id_extraction() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let endpoint_1 = discovery.spawn_endpoint().await?;
    let endpoint_2 = discovery.spawn_endpoint().await?;

    async fn handler(
        RemoteId(remote): RemoteId,
        State(expected): State<EndpointId>,
    ) -> impl IntoResponse {
        assert_eq!(remote, expected);
        "ok"
    }

    let app = Router::new()
        .route("/whoami", get(handler))
        .with_state(endpoint_2.id());
    let _router = iroh::protocol::Router::builder(endpoint_1.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();

    let client = IrohH3Client::new(endpoint_2, ALPN.into());
    let uri = format!("iroh+h3://{}/whoami", endpoint_1.id());

    let response = client
        .get(&uri)
        .send()
        .await
        .context("required operation value")?;
    assert_eq!(
        response.bytes().await.context("required operation value")?,
        b"ok"[..]
    );
    Ok(())
}
