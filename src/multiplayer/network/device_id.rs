//! Stable per-installation identifiers.
//!
//! * `device_id()` names this installation to the backend's seat lease
//!   (`/game/{id}/seat/claim`): the newest device to claim a player's seat
//!   takes it over and earlier devices become view-only.
//! * `wallet_gossip_seed()` is the Ed25519 seed that signs this wallet's
//!   gossip envelopes. It used to be regenerated every process start, so after
//!   a crash/restart the opponent saw a brand-new signer whose sequence
//!   restarted at 1 and rejected every gossip move as a causal gap. Persisting
//!   it per wallet keeps the opponent's per-signer lane continuous across
//!   restarts. It authorises gossip signing only — never funds.

use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{info, warn};

fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    let base = crate::core::paths::internal_data_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base
}

/// Matches the backend's `seat_lease::valid_device_id`.
pub fn is_valid_device_id(id: &str) -> bool {
    (8..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn device_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        // Separate instances on one machine (`XFCHESS_NODE_KEY_PATH`) are
        // separate devices for seat purposes too.
        let path = match std::env::var("XFCHESS_NODE_KEY_PATH") {
            Ok(p) if !p.trim().is_empty() => PathBuf::from(format!("{p}.device_id")),
            _ => data_dir().join("device_id"),
        };
        if let Ok(existing) = std::fs::read_to_string(&path) {
            let existing = existing.trim().to_string();
            if is_valid_device_id(&existing) {
                return existing;
            }
            warn!("[device] stored device id is malformed — regenerating");
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        if let Err(e) = std::fs::write(&path, &id) {
            warn!("[device] could not persist device id ({e}); seat resumes after restart will re-claim");
        }
        id
    })
}

/// Game id whose seat a write was refused for (`seat_superseded`), set from
/// HTTP worker threads and drained by the seat system on the main thread.
static SUPERSEDED_GAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn mark_seat_superseded(game_id: u64) {
    SUPERSEDED_GAME.store(game_id, std::sync::atomic::Ordering::Relaxed);
}

pub fn take_seat_superseded() -> Option<u64> {
    match SUPERSEDED_GAME.swap(0, std::sync::atomic::Ordering::Relaxed) {
        0 => None,
        id => Some(id),
    }
}

/// Whether an HTTP error body is the backend's seat-lease refusal.
pub fn is_seat_superseded_body(body: &str) -> bool {
    body.contains("seat_superseded")
}

fn gossip_seed_path(wallet: &str) -> PathBuf {
    data_dir().join(format!("gossip_seed_{wallet}"))
}

/// Load (or create and persist) the gossip-signing seed for `wallet`.
pub fn wallet_gossip_seed(wallet: &str) -> [u8; 32] {
    let path = gossip_seed_path(wallet);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(seed) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return seed;
        }
        warn!("[device] gossip seed for {wallet} is malformed — regenerating");
    }
    let seed: [u8; 32] = rand::random();
    match std::fs::write(&path, seed) {
        Ok(()) => info!("[device] created persistent gossip-signing seed for {wallet}"),
        Err(e) => warn!("[device] could not persist gossip seed ({e}); restart will change signer"),
    }
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_satisfy_the_backend_shape() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        assert!(is_valid_device_id(&id));
        assert!(!is_valid_device_id("short"));
        assert!(!is_valid_device_id("bad id with spaces"));
    }
}
