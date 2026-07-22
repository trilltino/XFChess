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
    // Override for running multiple instances on one machine (e.g. `just dev2`).
    // Without this, both instances load the SAME persisted node key → identical
    // node_id → the P2P relay can't tell host from joiner and misroutes JOIN_ACK,
    // so the host never detects the joiner. Prod is unaffected (different machines).
    if let Ok(p) = std::env::var("XFCHESS_NODE_KEY_PATH") {
        if !p.trim().is_empty() {
            let pb = PathBuf::from(p);
            if let Some(parent) = pb.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            return pb;
        }
    }
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

    let key = SecretKey::generate();
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

fn guest_username_path() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base.join("guest_username")
}

/// Load the locally-cached Guest display name, if one was ever saved. Guest
/// identity has no account and no server-side record — this is purely a
/// per-device display name shown to P2P peers. See
/// docs/plans/identity-implementation-plan.md.
pub fn load_guest_username() -> Option<String> {
    std::fs::read_to_string(guest_username_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Persist the Guest display name for next launch.
pub fn save_guest_username(name: &str) {
    if let Err(e) = std::fs::write(guest_username_path(), name.trim()) {
        warn!("[identity] Failed to save guest_username: {e}");
    }
}
