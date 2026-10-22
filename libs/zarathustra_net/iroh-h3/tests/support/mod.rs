use anyhow::Context as _;
use iroh::{Endpoint, address_lookup::MemoryLookup, endpoint::presets};

#[derive(Default)]
pub struct MockDiscoveryMap(MemoryLookup);

impl MockDiscoveryMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn spawn_endpoint(&self) -> anyhow::Result<Endpoint> {
        Ok({
            let endpoint = Endpoint::builder(presets::Minimal)
                .address_lookup(self.0.clone())
                .bind()
                .await
                .context("required operation value")?;
            self.0.add_endpoint_info(endpoint.addr());
            endpoint
        })
    }
}
