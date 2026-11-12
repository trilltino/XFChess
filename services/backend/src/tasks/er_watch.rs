use crate::signing::AppState;
use crate::telemetry::worker_metrics::{ER_LAST_PUSH_UNIX, ER_SUBSCRIPTION_CONNECTED};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use tracing::{info, warn};

const RECONNECT_BACKOFF: Duration = Duration::from_secs(10);

fn to_ws(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        url.to_string()
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn spawn_er_watch(state: Arc<AppState>) {
    tokio::spawn(async move {
        let program_id = match Pubkey::from_str(&state.config.program_id) {
            Ok(pk) => pk,
            Err(e) => {
                warn!("[er_watch] bad program_id ({e}) — real-time ER watch disabled");
                return;
            }
        };
        info!("[er_watch] starting Triton WS pubsub programSubscribe watch");

        loop {
            if let Err(e) = watch_once(&state, &program_id).await {
                warn!(
                    "[er_watch] subscription ended ({e}) — reconnecting in {}s",
                    RECONNECT_BACKOFF.as_secs()
                );
            }
            // Whether it never connected or dropped mid-stream, we're back to
            // poll-only until the next successful reconnect.
            ER_SUBSCRIPTION_CONNECTED.store(0, Ordering::Relaxed);
            tokio::time::sleep(RECONNECT_BACKOFF).await;
        }
    });
}

async fn watch_once(state: &Arc<AppState>, program_id: &Pubkey) -> anyhow::Result<()> {
    let ws_url = to_ws(&state.config.solana_rpc_url);
    let client = PubsubClient::new(&ws_url)
        .await
        .map_err(|e| anyhow::anyhow!("ws connect failed: {e}"))?;

    let config = RpcProgramAccountsConfig {
        account_config: RpcAccountInfoConfig {
            commitment: Some(CommitmentConfig::confirmed()),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut stream, _unsubscribe) = client
        .program_subscribe(program_id, Some(config))
        .await
        .map_err(|e| anyhow::anyhow!("program_subscribe failed: {e}"))?;

    ER_SUBSCRIPTION_CONNECTED.store(1, Ordering::Relaxed);
    info!("[er_watch] connected — watching program {program_id} for account pushes");

    while stream.next().await.is_some() {
        ER_LAST_PUSH_UNIX.store(now_unix(), Ordering::Relaxed);
    }

    Ok(()) // stream closed (server dropped us, or WS died) — caller reconnects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ws_converts_https_and_http() {
        assert_eq!(
            to_ws("https://xf.devnet.rpcpool.com/token"),
            "wss://xf.devnet.rpcpool.com/token"
        );
        assert_eq!(to_ws("http://127.0.0.1:8899"), "ws://127.0.0.1:8899");
    }

    #[test]
    fn to_ws_leaves_already_ws_urls_alone() {
        assert_eq!(to_ws("wss://already.ws/token"), "wss://already.ws/token");
    }
}
