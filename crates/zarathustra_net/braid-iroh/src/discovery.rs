use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use iroh::address_lookup::{
    AddressLookup, AddressLookupBuilder, EndpointData, EndpointInfo, Error, Item,
};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use n0_future::boxed::BoxStream;
use n0_future::StreamExt;

#[derive(Clone)]
pub enum DiscoveryConfig {
    Mock(MockDiscoveryMap),
    Real,
}

impl DiscoveryConfig {
    pub fn mock() -> Self {
        DiscoveryConfig::Mock(MockDiscoveryMap::new())
    }

    pub fn add_node(&self, node_addr: EndpointAddr) {
        if let DiscoveryConfig::Mock(map) = self {
            let info = EndpointInfo::from(node_addr);
            map.add_node(info.endpoint_id, info.data);
        }
        // DiscoveryConfig::Real doesn't need explicit registration - uses Iroh's built-in discovery
    }
}

#[derive(Debug, Default, Clone)]
pub struct MockDiscoveryMap {
    peers: Arc<RwLock<BTreeMap<EndpointId, Arc<EndpointData>>>>,
}

impl MockDiscoveryMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_node(&self, id: EndpointId, data: EndpointData) {
        self.peers.write().unwrap().insert(id, Arc::new(data));
    }
}

impl AddressLookupBuilder for MockDiscoveryMap {
    fn into_address_lookup(
        self,
        endpoint: &Endpoint,
    ) -> Result<impl AddressLookup, iroh::address_lookup::AddressLookupBuilderError> {
        Ok(MockDiscoveryInstance {
            id: endpoint.id(),
            map: self,
        })
    }
}

#[derive(Debug, Clone)]
struct MockDiscoveryInstance {
    id: EndpointId,
    map: MockDiscoveryMap,
}

impl AddressLookup for MockDiscoveryInstance {
    fn publish(&self, data: &EndpointData) {
        self.map
            .peers
            .write()
            .unwrap()
            .insert(self.id, Arc::new(data.clone()));
    }

    fn resolve(&self, endpoint_id: EndpointId) -> Option<BoxStream<Result<Item, Error>>> {
        let data = self.map.peers.read().unwrap().get(&endpoint_id).cloned()?;
        let info = EndpointInfo::from_parts(endpoint_id, data.as_ref().clone());
        let item = Item::new(info, "mock-braid", None);
        Some(n0_future::stream::once(Ok(item)).boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_discovery_map_new() {
        let map = MockDiscoveryMap::new();
        assert!(map.peers.read().unwrap().is_empty());
    }

    #[test]
    fn test_mock_discovery_default() {
        let map: MockDiscoveryMap = Default::default();
        assert!(map.peers.read().unwrap().is_empty());
    }

    #[test]
    fn test_discovery_config_mock() {
        let config = DiscoveryConfig::mock();
        matches!(config, DiscoveryConfig::Mock(_));
    }

    #[test]
    fn test_discovery_config_clone() {
        let config = DiscoveryConfig::mock();
        let cloned = config.clone();

        // Both should be Mock variant
        matches!(config, DiscoveryConfig::Mock(_));
        matches!(cloned, DiscoveryConfig::Mock(_));
    }

    #[test]
    fn test_mock_discovery_map_add_node() {
        let map = MockDiscoveryMap::new();
        let id = EndpointId::from_bytes(&[1u8; 32]).expect("valid test key");
        let data = EndpointData::default();

        map.add_node(id, data);

        assert_eq!(map.peers.read().unwrap().len(), 1);
        assert!(map.peers.read().unwrap().contains_key(&id));
    }

    #[test]
    fn test_mock_discovery_map_multiple_nodes() {
        let map = MockDiscoveryMap::new();

        for i in 0..5 {
            // Not every 32-byte pattern decompresses to a valid Edwards point (e.g. [0u8; 32]
            // doesn't), so derive a real public key from a secret key seed instead of treating
            // arbitrary bytes as a public key directly.
            let id = iroh::SecretKey::from_bytes(&[i as u8; 32]).public();
            let data = EndpointData::default();
            map.add_node(id, data);
        }

        assert_eq!(map.peers.read().unwrap().len(), 5);
    }
}
