use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use solana_sdk::signature::Keypair;

#[derive(Clone)]
pub struct FeepayerPool {
    keypairs: Arc<Vec<Keypair>>,
    counter: Arc<AtomicUsize>,
}

impl FeepayerPool {
    pub fn from_base58_list(keys: &[String], production: bool) -> anyhow::Result<Self> {
        let mut keypairs = Vec::with_capacity(keys.len());
        for key in keys {
            keypairs
                .push(super::load_keypair_from_env_value(key.trim()).map_err(anyhow::Error::msg)?);
        }
        if keypairs.is_empty() {
            anyhow::ensure!(
                !production,
                "FEE_PAYER_KEYS must contain a usable production key"
            );
            tracing::warn!("No FEE_PAYER_KEYS configured; using a temporary development key");
            keypairs.push(Keypair::new());
        }
        Ok(Self {
            keypairs: Arc::new(keypairs),
            counter: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn next(&self) -> &Keypair {
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.keypairs.len();
        &self.keypairs[idx]
    }

    pub fn all(&self) -> &[Keypair] {
        &self.keypairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_bad_keys_never_generate_replacement_credentials() {
        assert!(FeepayerPool::from_base58_list(&["not base58!".into()], false).is_err());
        assert!(FeepayerPool::from_base58_list(&["not base58!".into()], true).is_err());
        assert!(FeepayerPool::from_base58_list(&[], true).is_err());
        assert!(FeepayerPool::from_base58_list(&[], false).is_ok());
    }
}
