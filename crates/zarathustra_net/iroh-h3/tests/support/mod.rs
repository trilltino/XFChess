use iroh::{Endpoint, address_lookup::MemoryLookup, endpoint::presets};

#[derive(Default)]
pub struct MockDiscoveryMap(MemoryLookup);

impl MockDiscoveryMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn spawn_endpoint(&self) -> Endpoint {
        let endpoint = Endpoint::builder(presets::Minimal)
            .address_lookup(self.0.clone())
            .bind()
            .await
            .unwrap();
        self.0.add_endpoint_info(endpoint.addr());
        endpoint
    }
}
