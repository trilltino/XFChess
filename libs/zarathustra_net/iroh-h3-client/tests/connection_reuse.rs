use anyhow::Context as _;
#[path = "../../iroh-h3/tests/support/mod.rs"]
mod support;
use iroh_h3_axum::IrohAxum;
use iroh_h3_client::IrohH3Client;
use n0_future::{task::JoinSet, time::Instant};
use support::MockDiscoveryMap;

use axum::{Router, routing::post};

const ALPN: &[u8] = b"iroh+h3";

#[tokio::test]
async fn many_requests_connection_reuse() -> anyhow::Result<()> {
    let discovery = MockDiscoveryMap::new();
    let endpoint_1 = discovery.spawn_endpoint().await?;
    let endpoint_2 = discovery.spawn_endpoint().await?;

    async fn ping() -> &'static str {
        "Pong!"
    }

    let app = Router::new().route("/ping", post(ping));
    let _router = iroh::protocol::Router::builder(endpoint_1.clone())
        .accept(ALPN, IrohAxum::new(app))
        .spawn();

    let client = IrohH3Client::new(endpoint_2.clone(), ALPN.into());
    let uri = format!("iroh+h3://{}/ping", endpoint_1.id());

    for _ in 0..10 {
        let res = client
            .post(&uri)
            .send()
            .await
            .context("required operation value")?;
        assert_eq!(
            res.bytes().await.context("required operation value")?,
            b"Pong!"[..]
        );
    }

    let instant = Instant::now();
    let mut set = JoinSet::new();
    for _ in 0..50 {
        let request = client
            .post(&uri)
            .build()
            .context("required operation value")?;
        set.spawn(async move {
            let response = request.send().await.context("required operation value")?;
            response.bytes().await.context("required operation value")?;
            Ok::<_, anyhow::Error>(())
        });
    }
    while let Some(result) = set.join_next().await {
        result??;
    }
    println!("Burst processed in {:?}", instant.elapsed());
    Ok(())
}
