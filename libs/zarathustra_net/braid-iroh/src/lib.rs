pub mod discovery;
pub mod node;
mod persistence;
#[cfg(feature = "proxy")]
pub mod protocol;
#[cfg(feature = "proxy")]
pub mod proxy;
mod store;
pub mod subscription;

pub use discovery::*;
pub use node::*;
#[cfg(feature = "proxy")]
pub use protocol::AccessPolicy;
pub use subscription::*;

use std::sync::Arc;

pub struct BraidIrohState {
    pub peer: Arc<BraidIrohNode>,
    pub node_id: String,
    pub node_name: String,
}

pub async fn spawn_node(
    name: &str,
    port: Option<u16>,
    secret_key_override: Option<iroh::SecretKey>,
    discovery: DiscoveryConfig,
) -> anyhow::Result<Arc<BraidIrohState>> {
    let secret_key = if let Some(sk) = secret_key_override {
        sk
    } else {
        iroh::SecretKey::generate()
    };

    #[cfg(feature = "proxy")]
    let proxy_config = port.map(|port| crate::node::ProxyConfig {
        listen_addr: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        default_peer: secret_key.public(),
    });

    #[cfg(not(feature = "proxy"))]
    let proxy_config = None;
    #[cfg(not(feature = "proxy"))]
    let _ = port;

    let peer = BraidIrohNode::spawn(BraidIrohConfig {
        discovery,
        secret_key: Some(secret_key),
        proxy_config,
        data_dir: None,
    })
    .await?;

    let peer = Arc::new(peer);
    let peer_id = peer.node_id();

    tracing::info!("[INIT] Node ID: {}", peer_id);

    let state = Arc::new(BraidIrohState {
        peer,
        node_id: format!("{}", peer_id),
        node_name: name.to_string(),
    });

    Ok(state)
}
