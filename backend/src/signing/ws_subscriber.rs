use futures::{Stream, StreamExt, SinkExt};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, warn};

use crate::error::AppError;

/// Represents the cluster to connect to for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cluster {
    Localnet,
    Devnet,
    Mainnet,
}

impl Cluster {
    pub fn websocket_url(&self) -> &'static str {
        match self {
            Cluster::Localnet => "ws://localhost:8900",
            Cluster::Devnet => "wss://api.devnet.solana.com",
            Cluster::Mainnet => "wss://api.mainnet-beta.solana.com",
        }
    }
}

/// Represents an update to an account received via WebSocket subscription
#[derive(Debug, Clone)]
pub struct AccountUpdate {
    pub pubkey: Pubkey,
    pub data: Vec<u8>,
    pub slot: u64,
}

/// Manages WebSocket subscriptions to Solana accounts
pub struct WebSocketSubscriber {
    cluster: Cluster,
    /// Maximum number of subscriptions per connection to stay under free-tier limits
    max_subs_per_connection: usize,
    /// WebSocket connections for L1 subscriptions
    l1_connections: Vec<mpsc::Sender<(Pubkey, mpsc::Sender<AccountUpdate>)>>,
    /// WebSocket connections for ER subscriptions (if applicable)
    er_connections: Vec<mpsc::Sender<(Pubkey, mpsc::Sender<AccountUpdate>)>>,
    /// RPC client for fallback getAccountInfo calls on reconnect
    rpc_client: Arc<RpcClient>,
}

impl WebSocketSubscriber {
    /// Creates a new WebSocketSubscriber
    pub async fn new(
        cluster: Cluster,
        er_endpoint: Option<&str>,
        max_subs_per_connection: usize,
    ) -> Result<Self, AppError> {
        let rpc_url = match cluster {
            Cluster::Localnet => "http://localhost:8899".to_string(),
            Cluster::Devnet => "https://api.devnet.solana.com".to_string(),
            Cluster::Mainnet => "https://api.mainnet-beta.solana.com".to_string(),
        };
        let rpc_client = Arc::new(RpcClient::new(rpc_url));

        // Spawn two L1 WebSocket connections to split subscription load
        let mut l1_connections = Vec::new();
        for i in 0..2 {
            let (tx, rx) = mpsc::channel(100);
            let url = cluster.websocket_url().to_string();
            tokio::spawn(Self::manage_connection(url, rx, i));
            l1_connections.push(tx);
        }

        // Optionally spawn ER connections
        let mut er_connections = Vec::new();
        if let Some(er_url) = er_endpoint {
            for i in 0..2 {
                let (tx, rx) = mpsc::channel(100);
                let url = er_url.to_string();
                tokio::spawn(Self::manage_connection(url, rx, i + 2));
                er_connections.push(tx);
            }
        }

        Ok(Self {
            cluster,
            max_subs_per_connection,
            l1_connections,
            er_connections,
            rpc_client,
        })
    }

    /// Manages a single WebSocket connection, handling subscriptions and reconnections
    async fn manage_connection(
        url: String,
        mut rx: mpsc::Receiver<(Pubkey, mpsc::Sender<AccountUpdate>)>,
        connection_id: usize,
    ) {
        loop {
            info!("Connecting WebSocket {} to {}", connection_id, url);
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("WebSocket {} connected", connection_id);
                    let (mut write, mut read) = ws_stream.split();
                    let mut subscriptions = Vec::new();
                    let mut subscription_senders = Vec::new();

                    // Handle incoming subscription requests
                    while let Some((pubkey, sender)) = rx.recv().await {
                        if subscriptions.len() >= 100 {
                            warn!("Subscription limit reached on WebSocket {}", connection_id);
                            continue;
                        }

                        let subscription_id = subscriptions.len();
                        let subscribe_msg = format!(
                            r#"{{"jsonrpc":"2.0","id":{},"method":"accountSubscribe","params":["{}",{{ "encoding": "base64" }}]}}"#,
                            subscription_id,
                            pubkey
                        );
                        if let Err(e) = write.send(Message::Text(subscribe_msg)).await {
                            error!("Failed to send subscription message: {}", e);
                            return;
                        }

                        subscriptions.push(pubkey);
                        subscription_senders.push(sender);
                    }

                    // Handle incoming messages
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                // Parse JSON-RPC response
                                // Extract pubkey and data, forward to the appropriate sender
                                // This is a placeholder for actual JSON parsing logic
                                info!("Received message on WebSocket {}: {}", connection_id, text);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("WebSocket {} error: {}", connection_id, e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect WebSocket {}: {}", connection_id, e);
                }
            }

            // Reconnection logic with exponential backoff
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    /// Watches an account for updates on the specified cluster
    pub async fn watch_account(
        &self,
        pubkey: Pubkey,
        cluster: Cluster,
    ) -> Result<Pin<Box<dyn Stream<Item = AccountUpdate> + Send>>, AppError> {
        let (tx, rx) = mpsc::channel(100);

        // Choose L1 or ER connection based on cluster
        let connections = if cluster == self.cluster {
            &self.l1_connections
        } else {
            &self.er_connections
        };

        if connections.is_empty() {
            return Err(AppError::WebSocketSubscriptionError(
                "No ER connections available".to_string(),
            ));
        }

        // Simple load balancing: pick connection with least subscriptions
        // In a real implementation, track subscription count per connection
        let connection_index = 0; // Placeholder
        if let Err(e) = connections[connection_index].send((pubkey, tx)).await {
            return Err(AppError::WebSocketSubscriptionError(format!(
                "Failed to send subscription request: {}",
                e
            )));
        }

        // On reconnect, fetch latest account state via RPC to catch missed updates
        // This is a placeholder for actual RPC call
        // let initial_state = self.rpc_client.get_account(&pubkey)?;

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_subscriber_new() {
        let subscriber = WebSocketSubscriber::new(Cluster::Localnet, None, 100).await;
        assert!(subscriber.is_ok());
    }
}
