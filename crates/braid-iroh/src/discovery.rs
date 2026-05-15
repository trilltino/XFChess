//! Peer discovery configuration for braid_iroh.
//!
//! Uses Iroh's real discovery (DNS + Pkarr + MDNS) for all environments.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use iroh::address_lookup::{
    AddressLookup, EndpointData, EndpointInfo, Error, IntoAddressLookup, Item,
};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use n0_future::boxed::BoxStream;
use n0_future::StreamExt;

/// How peers find each other on network.
#[derive(Clone)]
pub enum DiscoveryConfig {
    /// Use Iroh's real discovery (DNS, Pkarr, MDNS).
    Real,
}

impl DiscoveryConfig {
    /// Register a node's address so other peers can find it.
    pub fn add_node(&self, node_addr: EndpointAddr) {
        // DiscoveryConfig::Real doesn't need explicit registration - uses Iroh's built-in discovery
    }
}