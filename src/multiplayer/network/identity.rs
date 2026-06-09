//! Stable Iroh node key persistence.
//!
//! Stored at `$config_dir/xfchess/node_key` as 32 raw bytes.
//! On first run a random key is generated and saved; subsequent runs
//! reload it so the node ID stays constant across restarts and wallet
//! changes — the **social identity anchor**.

use iroh::SecretKey;
use std::path::PathBuf;
use tracing::{info, warn};

fn key_path() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base.join("node_key")
}

/// Load the persisted node secret key, or generate + save a new one.
pub fn load_or_create() -> SecretKey {
    let path = key_path();

    if path.exists() {
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                let key = SecretKey::from_bytes(&arr);
                info!("[identity] Loaded persistent node key from {:?}", path);
                return key;
            }
            Ok(_) => warn!("[identity] node_key file wrong length — regenerating"),
            Err(e) => warn!("[identity] Failed to read node_key: {e} — regenerating"),
        }
    }

    let key = SecretKey::generate(&mut rand::rng());
    let bytes = key.to_bytes();
    if let Err(e) = std::fs::write(&path, bytes) {
        warn!("[identity] Failed to save node_key: {e}");
    } else {
        info!("[identity] Generated new node key, saved to {:?}", path);
    }
    key
}

/// Return the base58-encoded public node ID (matches the Iroh `EndpointId` format).
pub fn node_id_b58() -> String {
    let key = load_or_create();
    let public = key.public();
    bs58::encode(public.as_bytes()).into_string()
}
