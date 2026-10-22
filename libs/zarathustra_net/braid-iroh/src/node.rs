use braid_http::Update;

use crate::store::ResourceStore;
use iroh::{endpoint::presets, protocol::Router, Endpoint, EndpointAddr, EndpointId, SecretKey};
use iroh_gossip::api::GossipReceiver;
use iroh_gossip::net::Gossip;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::discovery::{DiscoveryConfig, MockDiscoveryMap};
#[cfg(feature = "proxy")]
use crate::protocol::{self, BraidAppState};
use crate::subscription::SubscriptionManager;

pub const BRAID_H3_ALPN: &[u8] = b"braid-h3/0";

#[derive(Clone)]
pub struct BraidIrohConfig {
    pub discovery: DiscoveryConfig,

    pub secret_key: Option<SecretKey>,

    pub data_dir: Option<PathBuf>,

    pub proxy_config: Option<ProxyConfig>,
}

#[derive(Clone)]
pub struct ProxyConfig {
    pub listen_addr: std::net::SocketAddr,
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

pub struct BraidIrohNode {
    endpoint: Endpoint,
    router: Router,
    subscription_mgr: Arc<SubscriptionManager>,
    resources: Arc<ResourceStore>,
    data_dir: Option<PathBuf>,
    proxy_task: Option<JoinHandle<()>>,
    flush_task: Option<crate::persistence::FlushTask>,
    proxy_addr: Option<std::net::SocketAddr>,
}

impl BraidIrohNode {
    pub async fn spawn(config: BraidIrohConfig) -> anyhow::Result<Self> {
        let mut builder = match config.discovery {
            DiscoveryConfig::Mock(map) => Endpoint::builder(presets::Minimal).address_lookup(map),
            DiscoveryConfig::Real => Endpoint::builder(presets::N0),
        };
        if let Some(key) = config.secret_key {
            builder = builder.secret_key(key);
        }
        #[cfg(not(feature = "proxy"))]
        anyhow::ensure!(
            config.proxy_config.is_none(),
            "TCP bridge requires the proxy feature"
        );
        let endpoint = builder.bind().await?;

        tracing::info!(id = %endpoint.id(), "iroh endpoint bound");

        let initial_data = if let Some(ref dir) = config.data_dir {
            tokio::fs::create_dir_all(dir).await?;
            crate::persistence::load(dir).await?
        } else {
            Default::default()
        };
        let resources = Arc::new(ResourceStore::new(initial_data));
        #[cfg(feature = "proxy")]
        let proxy_listener = match &config.proxy_config {
            Some(proxy) => Some(tokio::net::TcpListener::bind(proxy.listen_addr).await?),
            None => None,
        };
        #[cfg(feature = "proxy")]
        let proxy_addr = proxy_listener
            .as_ref()
            .map(|listener| listener.local_addr())
            .transpose()?;
        #[cfg(not(feature = "proxy"))]
        let proxy_addr = None;
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let subscription_mgr = Arc::new(SubscriptionManager::new(gossip.clone()));
        #[cfg(feature = "proxy")]
        let app_state = BraidAppState {
            subscriptions: subscription_mgr.clone(),
            resources: resources.clone(),
        };

        #[cfg(feature = "proxy")]
        let router = {
            let braid_handler = protocol::build_protocol_handler(app_state.clone());
            Router::builder(endpoint.clone())
                .accept(BRAID_H3_ALPN, braid_handler)
                .accept(iroh_gossip::net::GOSSIP_ALPN, gossip)
                .spawn()
        };
        #[cfg(not(feature = "proxy"))]
        let router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::net::GOSSIP_ALPN, gossip)
            .spawn();

        #[cfg(feature = "proxy")]
        let proxy_task = if let (Some(proxy_conf), Some(listener)) =
            (config.proxy_config, proxy_listener)
        {
            let endpoint = endpoint.clone();
            let local_router = protocol::router(app_state);
            Some(tokio::spawn(async move {
                if let Err(e) =
                    crate::proxy::serve(endpoint, listener, proxy_conf.default_peer, local_router)
                        .await
                {
                    tracing::error!("TCP bridge failed: {e}");
                }
            }))
        } else {
            None
        };
        #[cfg(not(feature = "proxy"))]
        let proxy_task = None;
        let flush_task = config
            .data_dir
            .as_ref()
            .map(|dir| crate::persistence::FlushTask::spawn(dir.clone(), resources.clone()));
        Ok(Self {
            endpoint,
            router,
            subscription_mgr,
            resources,
            data_dir: config.data_dir,
            proxy_task,
            flush_task,
            proxy_addr,
        })
    }

    pub fn node_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub async fn node_addr(&self) -> anyhow::Result<EndpointAddr> {
        Ok(self.endpoint.addr())
    }

    pub async fn subscribe(
        &self,
        url: &str,
        bootstrap: Vec<EndpointId>,
    ) -> anyhow::Result<GossipReceiver> {
        tracing::debug!(
            url,
            bootstrap_count = bootstrap.len(),
            bootstrap = ?bootstrap,
            "subscribing"
        );
        let (_sender, receiver) = self.subscription_mgr.subscribe(url, bootstrap).await?;
        tracing::debug!(url, "subscribed successfully");
        Ok(receiver)
    }

    pub async fn put(&self, url: &str, update: Update) -> anyhow::Result<()> {
        self.resources.append(url, update.clone()).await;
        self.subscription_mgr.broadcast(url, &update).await
    }

    /// Retain an update for local HTTP/bridge subscribers while sending its body
    /// directly to gossip peers that understand the application's binary framing.
    pub async fn put_raw(&self, url: &str, update: Update) -> anyhow::Result<()> {
        let body = update
            .body
            .clone()
            .ok_or_else(|| anyhow::anyhow!("raw gossip requires an update body"))?;
        self.resources.append(url, update).await;
        self.subscription_mgr.broadcast_raw(url, body).await
    }

    pub async fn store_update(&self, url: &str, update: Update) {
        self.resources.append(url, update).await;
    }

    pub async fn get(&self, url: &str) -> Option<Update> {
        self.resources.latest(url).await
    }

    pub async fn get_version(&self, url: &str, version_id: &str) -> Option<Update> {
        self.resources.version(url, version_id).await
    }

    pub async fn get_history(&self, url: &str) -> Vec<String> {
        self.resources
            .history(url)
            .await
            .iter()
            .map(|u| {
                u.version
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect()
    }

    pub async fn get_updates(&self, url: &str) -> Vec<Update> {
        self.resources.history(url).await
    }

    pub async fn get_updates_since(&self, url: &str, since_version: &str) -> Vec<Update> {
        let history = self.resources.history(url).await;
        let offset = history
            .iter()
            .position(|u| u.version.iter().any(|v| v.to_string() == since_version))
            .map_or(0, |i| i + 1);
        history.into_iter().skip(offset).collect()
    }

    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(task) = self.proxy_task.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(task) = self.flush_task.as_mut() {
            task.shutdown().await?;
        }
        let result = if let Some(dir) = &self.data_dir {
            crate::persistence::save(dir, &self.resources.snapshot().await).await
        } else {
            Ok(())
        };
        self.router.shutdown().await?;
        result
    }

    pub fn subscriptions(&self) -> &Arc<SubscriptionManager> {
        &self.subscription_mgr
    }
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn proxy_addr(&self) -> Option<std::net::SocketAddr> {
        self.proxy_addr
    }

    pub async fn join_peers(&self, url: &str, peers: Vec<EndpointId>) -> anyhow::Result<()> {
        tracing::debug!(url, peer_count = peers.len(), peers = ?peers, "joining peers to topic");
        self.subscription_mgr.join_peers(url, peers).await
    }
}

impl Drop for BraidIrohNode {
    fn drop(&mut self) {
        if let Some(task) = &self.proxy_task {
            task.abort();
        }
    }
}
