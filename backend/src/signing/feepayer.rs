use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use solana_sdk::signature::Keypair;

fn load_keypair_from_file(path: &str) -> Option<Keypair> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| tracing::error!("[FeepayerPool] cannot read keypair file {path}: {e}"))
        .ok()?;
    let bytes: Vec<u8> = serde_json::from_str(&text)
        .map_err(|e| tracing::error!("[FeepayerPool] invalid JSON in {path}: {e}"))
        .ok()?;
    Keypair::try_from(bytes.as_slice())
        .map_err(|e| tracing::error!("[FeepayerPool] invalid keypair bytes in {path}: {e}"))
        .ok()
}

#[derive(Clone)]
pub struct FeepayerPool {
    keypairs: Arc<Vec<Keypair>>,
    counter: Arc<AtomicUsize>,
}

impl FeepayerPool {
    pub fn from_base58_list(keys: &[String]) -> Self {
        let keypairs: Vec<Keypair> = keys
            .iter()
            .filter_map(|k| {
                let k = k.trim();
                if k.ends_with(".json")
                    || k.starts_with('.')
                    || k.starts_with('/')
                    || k.contains('\\')
                {
                    load_keypair_from_file(k)
                } else {
                    let bytes = bs58::decode(k).into_vec().ok()?;
                    Keypair::try_from(bytes.as_slice()).ok()
                }
            })
            .collect();

        let keypairs = if keypairs.is_empty() {
            tracing::warn!(
                "[FeepayerPool] No FEE_PAYER_KEYS set — using ephemeral keypair (dev only)"
            );
            vec![Keypair::new()]
        } else {
            keypairs
        };

        Self {
            keypairs: Arc::new(keypairs),
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn next(&self) -> &Keypair {
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.keypairs.len();
        &self.keypairs[idx]
    }

    pub fn all(&self) -> &[Keypair] {
        &self.keypairs
    }
}
