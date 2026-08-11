//! Stable Iroh node key persistence.
//!
//! Stored at `$config_dir/xfchess/node_key` as 32 raw bytes.
//! On first run a random key is generated and saved; subsequent runs
//! reload it so the node ID stays constant across restarts and wallet
//! changes — the **social identity anchor**.

use iroh::SecretKey;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{info, warn};

/// Arbitrary fixed loopback port used purely as an OS-level exclusivity
/// mutex — never actually served. Binding it is how a process claims
/// "I am the one persisted node identity on this machine right now".
const IDENTITY_MUTEX_PORT: u16 = 47771;

/// Holds the bound listener for the process's lifetime when we're the
/// primary instance. Never accepted on; its only job is to keep the port
/// held so a second launch's bind attempt fails. The OS reclaims it
/// automatically on exit or crash — no stale-lock cleanup needed, unlike a
/// PID file.
static IDENTITY_MUTEX: OnceLock<Option<TcpListener>> = OnceLock::new();

/// A same-process-only identity used when another instance already holds
/// the persisted one. Cached so repeated calls to `load_or_create()` within
/// this process return the same key instead of a fresh random one each time.
static FALLBACK_KEY: OnceLock<SecretKey> = OnceLock::new();

/// True if this process holds the persisted node identity (i.e. no other
/// instance was already running when we started).
fn is_primary_instance() -> bool {
    IDENTITY_MUTEX
        .get_or_init(|| TcpListener::bind(("127.0.0.1", IDENTITY_MUTEX_PORT)).ok())
        .is_some()
}

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
///
/// If another instance is already running on this machine and holds the
/// persisted identity, returns a same-process-only fallback key instead —
/// two live instances sharing one node ID isn't a config a player can even
/// see, let alone fix, and it silently breaks P2P between them (the relay
/// rejects the second connection as a duplicate endpoint).
pub fn load_or_create() -> SecretKey {
    if !is_primary_instance() {
        return FALLBACK_KEY
            .get_or_init(|| {
                let key = SecretKey::generate();
                warn!(
                    "[identity] Another XFChess instance is already running — using a \
                     temporary identity for this one (not persisted; P2P between the \
                     two won't use your usual node ID)."
                );
                key
            })
            .clone();
    }

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
    // Documents, not config_dir — the same `Documents/xfchess/` folder the
    // Save-PGN feature already writes to, so local player data lives in one
    // discoverable place instead of split between a visible and a hidden dir.
    let base = dirs::document_dir()
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

// ── Multiple local profiles ─────────────────────────────────────────────────
//
// Several people can share one machine, each with their own name and PGN
// history. `profiles.json` lists every profile ever created plus which one
// is active; each profile's PGN files live in their own subfolder so games
// never mix. The pre-existing single `guest_username` file above is kept as
// legacy/fallback source data — on first read, if `profiles.json` doesn't
// exist yet but a `guest_username` does, it's migrated in as the first
// profile automatically so returning players don't see an empty picker.

fn profiles_dir() -> PathBuf {
    let base = dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base
}

fn profiles_path() -> PathBuf {
    profiles_dir().join("profiles.json")
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ProfilesFile {
    names: Vec<String>,
    active: Option<String>,
}

fn read_profiles_file() -> ProfilesFile {
    match std::fs::read_to_string(profiles_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => {
            // Migrate the legacy single-profile file, if any, so an existing
            // player's name and PGN history keep working after the update.
            match load_guest_username() {
                Some(name) => ProfilesFile {
                    names: vec![name.clone()],
                    active: Some(name),
                },
                None => ProfilesFile::default(),
            }
        }
    }
}

fn write_profiles_file(file: &ProfilesFile) {
    if let Ok(json) = serde_json::to_string_pretty(file) {
        if let Err(e) = std::fs::write(profiles_path(), json) {
            warn!("[identity] Failed to save profiles.json: {e}");
        }
    }
}

/// Names of every local profile ever created, in creation order.
pub fn list_profiles() -> Vec<String> {
    read_profiles_file().names
}

/// The profile active as of the last launch, if any.
pub fn load_active_profile() -> Option<String> {
    read_profiles_file().active
}

/// Create a new local profile (no-op if the name already exists) and make it
/// the active one. Also ensures its PGN subfolder exists.
pub fn create_profile(name: &str) {
    let name = name.trim().to_string();
    if name.is_empty() {
        return;
    }
    let mut file = read_profiles_file();
    if !file.names.iter().any(|n| n == &name) {
        file.names.push(name.clone());
    }
    file.active = Some(name.clone());
    write_profiles_file(&file);
    // Keep the legacy single-file path in sync for any code that hasn't
    // moved to the multi-profile API yet.
    save_guest_username(&name);
    ensure_profile_pgn_dir(&name);
}

/// Switch the active profile to an already-existing name (no-op if unknown).
pub fn set_active_profile(name: &str) {
    let mut file = read_profiles_file();
    if !file.names.iter().any(|n| n == name) {
        return;
    }
    file.active = Some(name.to_string());
    write_profiles_file(&file);
    save_guest_username(name);
    ensure_profile_pgn_dir(name);
}

/// Filesystem-safe folder name for a profile — replaces path separators and
/// other characters that would otherwise escape `profiles/` or fail on
/// Windows.
fn sanitize_profile_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// PGN save folder for a given profile, e.g.
/// `Documents/xfchess/profiles/<name>/`. Created on demand.
pub fn profile_pgn_dir(name: &str) -> PathBuf {
    profiles_dir()
        .join("profiles")
        .join(sanitize_profile_name(name))
}

fn ensure_profile_pgn_dir(name: &str) {
    std::fs::create_dir_all(profile_pgn_dir(name)).ok();
}
