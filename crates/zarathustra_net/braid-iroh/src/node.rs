//! BraidIrohNode — the main entry point for a Braid-over-Iroh peer.
//!
//! Orchestrates an iroh `Endpoint`, a gossip `Gossip` instance, and the
//! Braid protocol handler. This is the type you create to participate
//! in the P2P Braid network.

use braid_core::Update;

use iroh::{protocol::Router, Endpoint, EndpointAddr, EndpointId, SecretKey};
use iroh_gossip::api::GossipReceiver;
use iroh_gossip::net::Gossip;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::discovery::{DiscoveryConfig, MockDiscoveryMap};
#[cfg(feature = "proxy")]
use crate::protocol::{self, BraidAppState};
use crate::subscription::SubscriptionManager;

/// ALPN protocol identifier for Braid-over-H3.
/// Peers negotiate this during the QUIC handshake.
pub const BRAID_H3_ALPN: &[u8] = b"braid-h3/0";

/// Configuration for spawning a BraidIrohNode.
#[derive(Clone)]
pub struct BraidIrohConfig {
    /// Discovery configuration.
    pub discovery: DiscoveryConfig,

    /// Optional pre-generated secret key for a stable identity.
    /// If `None`, a random identity is generated.
    pub secret_key: Option<SecretKey>,

    /// Optional directory for durable resource persistence.
    /// When set, every `put()` is written to `<data_dir>/resources.json`
    /// and the store is reloaded from disk on startup.
    pub data_dir: Option<PathBuf>,

    /// Optional configuration for the TCP proxy bridge.
    pub proxy_config: Option<ProxyConfig>,
}


/// Configuration for the TCP proxy bridge.
#[derive(Clone)]
pub struct ProxyConfig {
    /// Local address to listen on (e.g. 127.0.0.1:8080).
    pub listen_addr: std::net::SocketAddr,
    /// Default peer ID to forward requests to.
    pub default_peer: EndpointId,
}

impl Default for BraidIrohConfig {
    fn default() -> Self {
        Self {
            discovery: DiscoveryConfig::Mock(MockDiscoveryMap::new()),
            secret_key: None,
            proxy_config: None,
            data_dir: None,
        }
    }
}

/// Load the persisted resource map from `data_dir/resources.json`.
/// Returns an empty map if the file doesn't exist or is corrupt.
async fn load_resources(data_dir: &Path) -> HashMap<String, Vec<Update>> {
    let path = data_dir.join("resources.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Flush the full resource map to `data_dir/resources.json` atomically
/// (write to a temp file then rename).
async fn save_resources(data_dir: &Path, map: &HashMap<String, Vec<Update>>) {
    let path = data_dir.join("resources.json");
    let tmp = data_dir.join("resources.json.tmp");
    let Ok(json) = serde_json::to_string(map) else { return };
    if tokio::fs::write(&tmp, &json).await.is_ok() {
        tokio::fs::rename(&tmp, &path).await.ok();
    }
}

/// A Braid-capable P2P peer. Holds the iroh endpoint, gossip, and
/// subscription state. Create one per peer identity.
pub struct BraidIrohNode {
    endpoint: Endpoint,
    #[allow(dead_code)]
    router: Router,
    subscription_mgr: Arc<SubscriptionManager>,
    resources: Arc<RwLock<HashMap<String, Vec<Update>>>>,
    /// Optional directory for durable persistence of the resource store.
    data_dir: Option<PathBuf>,
}

impl BraidIrohNode {
    /// Spawn a new Braid-over-Iroh node.
    ///
    /// This sets up the iroh endpoint, starts the gossip protocol,
    /// mounts the Braid-HTTP Axum routes via `IrohAxum`, and begins
    /// accepting incoming connections.
    pub async fn spawn(config: BraidIrohConfig) -> anyhow::Result<Self> {
        // 1. Build the iroh endpoint with discovery + Braid ALPN
        let mut builder = Endpoint::builder().alpns(vec![
            BRAID_H3_ALPN.to_vec(),
            iroh_gossip::net::GOSSIP_ALPN.to_vec(),
        ]);

        // Apply optional secret key
        if let Some(key) = config.secret_key {
            builder = builder.secret_key(key);
        }

        // Apply discovery logic
        match config.discovery {
            DiscoveryConfig::Mock(map) => {
                builder = builder.address_lookup(map);
            }
            DiscoveryConfig::Real => {
                // Default Iroh discovery is already enabled by default in Endpoint::builder()
                // unless we override it. So we do nothing here.
            }
        }

        let endpoint = builder.bind().await?;

        tracing::info!(id = %endpoint.id(), "iroh endpoint bound");

        // 2. Start the gossip protocol on this endpoint
        // In iroh 0.96, spawning gossip is synchronous and returns the Gossip handle directly
        let gossip = Gossip::builder().spawn(endpoint.clone());

        // 3. Build shared state — load from disk if data_dir is configured
        let initial_data = if let Some(ref dir) = config.data_dir {
            tokio::fs::create_dir_all(dir).await.ok();
            let loaded = load_resources(dir).await;
            tracing::info!("[braid] Loaded {} resource URLs from disk", loaded.len());
            loaded
        } else {
            HashMap::new()
        };
        let resources: Arc<RwLock<HashMap<String, Vec<Update>>>> =
            Arc::new(RwLock::new(initial_data));

        let subscription_mgr = Arc::new(SubscriptionManager::new(gossip.clone()));

        // 4. Mount the Braid protocol handler on the iroh router (only when proxy feature is enabled)
        #[cfg(feature = "proxy")]
        let router = {
            let app_state = BraidAppState {
                subscriptions: subscription_mgr.clone(),
                resources: resources.clone(),
            };
            let braid_handler = protocol::build_protocol_handler(app_state);
            Router::builder(endpoint.clone())
                .accept(BRAID_H3_ALPN.to_vec(), braid_handler)
                .accept(iroh_gossip::net::GOSSIP_ALPN.to_vec(), gossip.clone())
                .spawn()
        };
        #[cfg(not(feature = "proxy"))]
        let router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::net::GOSSIP_ALPN.to_vec(), gossip.clone())
            .spawn();

        // 5. Start TCP Proxy if configured (Phase 4)
        #[cfg(feature = "proxy")]
        if let Some(proxy_conf) = config.proxy_config {
            let endpoint_clone = endpoint.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::proxy::bridge::start_proxy(
                    &endpoint_clone,
                    proxy_conf.listen_addr,
                    proxy_conf.default_peer,
                )
                .await
                {
                    tracing::error!("TCP Proxy failed: {}", e);
                }
            });
        }

        Ok(Self {
            endpoint,
            router,
            subscription_mgr,
            resources,
            data_dir: config.data_dir,
        })
    }

    /// This node's public identity (EndpointId / NodeId).
    pub fn node_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    /// Full address info for this node (id + addresses + relay).
    pub async fn node_addr(&self) -> anyhow::Result<EndpointAddr> {
        Ok(self.endpoint.addr())
    }

    /// Subscribe to a resource URL on the gossip network.
    /// Returns a receiver stream of gossip events for this resource.
    pub async fn subscribe(
        &self,
        url: &str,
        bootstrap: Vec<EndpointId>,
    ) -> anyhow::Result<GossipReceiver> {
        println!("[NODE] Subscribing to {} with {} bootstrap peers", url, bootstrap.len());
        for peer in &bootstrap {
            println!("[NODE]   bootstrap: {}", peer);
        }
        let (_sender, receiver) = self.subscription_mgr.subscribe(url, bootstrap).await?;
        println!("[NODE] Subscribed successfully to {}", url);
        Ok(receiver)
    }

    /// PUT a Braid Update to a resource. Stores it locally and broadcasts
    /// to all gossip subscribers.
    pub async fn put(&self, url: &str, update: Update) -> anyhow::Result<()> {
        // Normalize the URL for consistent storage (using same logic as SubscriptionManager)
        let normalized = crate::subscription::SubscriptionManager::normalize_url(url);
        
        // Debug logging for Braid format
        println!("\nOUTGOING BRAID PUT:");
        println!("PUT {} HTTP/3", normalized);
        println!("Version: {:?}", update.version);
        if !update.parents.is_empty() {
            println!("Parents: {:?}", update.parents);
        }
        if let Some(body) = &update.body {
            println!("Content-Length: {}", body.len());
            println!();
            println!("{}", String::from_utf8_lossy(body));
        }
        println!("----------------------------------------\n");

        {
            let mut guard = self.resources.write().await;
            guard
                .entry(normalized.clone())
                .or_insert_with(Vec::new)
                .push(update.clone());
            // Flush to disk if persistence is configured.
            if let Some(ref dir) = self.data_dir {
                let snapshot: HashMap<String, Vec<Update>> = guard.clone();
                let dir = dir.clone();
                tokio::spawn(async move { save_resources(&dir, &snapshot).await });
            }
        }
        self.subscription_mgr.broadcast(&normalized, &update).await?;
        Ok(())
    }

    /// Store an update locally without broadcasting (for received gossip).
    pub async fn store_update(&self, url: &str, update: Update) {
        self.resources
            .write()
            .await
            .entry(url.to_string())
            .or_insert_with(Vec::new)
            .push(update);
    }

    /// GET the latest state of a resource from local storage.
    #[allow(dead_code)]
    pub async fn get(&self, url: &str) -> Option<Update> {
        self.resources
            .read()
            .await
            .get(url)
            .and_then(|h| h.last().cloned())
    }

    /// GET a specific version of a resource.
    pub async fn get_version(&self, url: &str, version_id: &str) -> Option<Update> {
        self.resources.read().await.get(url).and_then(|history| {
            history
                .iter()
                .find(|u| u.version.iter().any(|v| v.to_string().contains(version_id)))
                .cloned()
        })
    }

    /// GET all version IDs for a resource (latest last).
    pub async fn get_history(&self, url: &str) -> Vec<String> {
        if let Some(history) = self.resources.read().await.get(url) {
            history
                .iter()
                .map(|u| {
                    u.version
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// GET all stored Updates for a resource URL, in order (oldest first).
    pub async fn get_updates(&self, url: &str) -> Vec<Update> {
        let normalized = SubscriptionManager::normalize_url(url);
        self.resources
            .read()
            .await
            .get(&normalized)
            .cloned()
            .unwrap_or_default()
    }

    /// GET all Updates for a resource that were stored after `since_version`.
    ///
    /// Walks the stored history and returns everything following the first
    /// entry whose primary version string matches `since_version`.  If
    /// `since_version` is not found, the full history is returned so the
    /// caller always ends up in a consistent state.
    pub async fn get_updates_since(&self, url: &str, since_version: &str) -> Vec<Update> {
        let normalized = SubscriptionManager::normalize_url(url);
        let guard = self.resources.read().await;
        let Some(history) = guard.get(&normalized) else {
            return Vec::new();
        };
        let pos = history.iter().position(|u| {
            u.version
                .first()
                .map(|v| v.to_string() == since_version)
                .unwrap_or(false)
        });
        match pos {
            Some(idx) => history[idx + 1..].to_vec(),
            None => history.clone(),
        }
    }

    /// Shut down the node gracefully.
    #[allow(dead_code)]
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.router.shutdown().await?;
        Ok(())
    }

    /// Access the subscription manager (for advanced usage).
    #[allow(dead_code)]
    pub fn subscriptions(&self) -> &Arc<SubscriptionManager> {
        &self.subscription_mgr
    }

    /// Access the iroh endpoint (for advanced usage).
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Join additional peers to an existing gossip topic.
    pub async fn join_peers(&self, url: &str, peers: Vec<EndpointId>) -> anyhow::Result<()> {
        println!("[NODE] Joining {} peers to topic {}", peers.len(), url);
        for peer in &peers {
            println!("[NODE]   joining: {}", peer);
        }
        self.subscription_mgr.join_peers(url, peers).await
    }
}
