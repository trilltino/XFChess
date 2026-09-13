use iroh::SecretKey;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{info, warn};

const IDENTITY_MUTEX_PORT: u16 = 47771;

static IDENTITY_MUTEX: OnceLock<Option<TcpListener>> = OnceLock::new();

static FALLBACK_KEY: OnceLock<SecretKey> = OnceLock::new();

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
    #[cfg(target_os = "android")]
    let base = crate::core::paths::internal_data_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base.join("node_key")
}

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

pub fn node_id_b58() -> String {
    let key = load_or_create();
    let public = key.public();
    bs58::encode(public.as_bytes()).into_string()
}

fn guest_username_path() -> PathBuf {
    // Documents, not config_dir — the same `Documents/xfchess/` folder the
    // Save-PGN feature already writes to, so local player data lives in one
    // discoverable place instead of split between a visible and a hidden dir.
    // Android has no equivalent shared/visible Documents folder without the
    // Storage Access Framework (out of scope for v1); `external_data_dir()`
    // is the closest analog — still app-scoped, but visible via a file
    // manager and where the PGN-save fallback (game_over_popup.rs) also lands.
    #[cfg(target_os = "android")]
    let base = crate::core::paths::external_data_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base.join("guest_username")
}

pub fn load_guest_username() -> Option<String> {
    std::fs::read_to_string(guest_username_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

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
    #[cfg(target_os = "android")]
    let base = crate::core::paths::external_data_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess");
    std::fs::create_dir_all(&base).ok();
    base
}

fn profiles_path() -> PathBuf {
    profiles_dir().join("profiles.json")
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct ProfileEntry {
    pub name: String,
    pub save_path: Option<PathBuf>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ProfilesFile {
    #[serde(default)]
    profiles: Vec<ProfileEntry>,
    #[serde(default)]
    names: Vec<String>,
    active: Option<String>,
}

fn read_profiles_file() -> ProfilesFile {
    let mut file = match std::fs::read_to_string(profiles_path()) {
        Ok(s) => serde_json::from_str::<ProfilesFile>(&s).unwrap_or_default(),
        Err(_) => {
            // Migrate the legacy single-profile file, if any, so an existing
            // player's name and PGN history keep working after the update.
            match load_guest_username() {
                Some(name) => ProfilesFile {
                    profiles: vec![ProfileEntry {
                        name: name.clone(),
                        save_path: None,
                    }],
                    names: vec![],
                    active: Some(name),
                },
                None => ProfilesFile::default(),
            }
        }
    };

    // Migrate legacy `names` array to `profiles` array.
    if !file.names.is_empty() {
        for name in file.names.drain(..) {
            if !file.profiles.iter().any(|p| p.name == name) {
                file.profiles.push(ProfileEntry {
                    name,
                    save_path: None,
                });
            }
        }
        write_profiles_file(&file);
    }

    file
}

fn write_profiles_file(file: &ProfilesFile) {
    if let Ok(json) = serde_json::to_string_pretty(file) {
        if let Err(e) = std::fs::write(profiles_path(), json) {
            warn!("[identity] Failed to save profiles.json: {e}");
        }
    }
}

pub fn list_profiles() -> Vec<String> {
    read_profiles_file()
        .profiles
        .into_iter()
        .map(|p| p.name)
        .collect()
}

pub fn load_active_profile() -> Option<String> {
    read_profiles_file().active
}

pub fn create_profile(name: &str, save_path: Option<PathBuf>) {
    let name = name.trim().to_string();
    if name.is_empty() {
        return;
    }
    let mut file = read_profiles_file();
    if !file.profiles.iter().any(|p| p.name == name) {
        file.profiles.push(ProfileEntry {
            name: name.clone(),
            save_path,
        });
    }
    file.active = Some(name.clone());
    write_profiles_file(&file);
    // Keep the legacy single-file path in sync for any code that hasn't
    // moved to the multi-profile API yet.
    save_guest_username(&name);
    ensure_profile_pgn_dir(&name);
}

pub fn set_active_profile(name: &str) {
    let mut file = read_profiles_file();
    if !file.profiles.iter().any(|p| p.name == name) {
        return;
    }
    file.active = Some(name.to_string());
    write_profiles_file(&file);
    save_guest_username(name);
    ensure_profile_pgn_dir(name);
}

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

pub fn profile_pgn_dir(name: &str) -> PathBuf {
    let file = read_profiles_file();
    if let Some(profile) = file.profiles.iter().find(|p| p.name == name) {
        if let Some(path) = &profile.save_path {
            return path.clone();
        }
    }
    profiles_dir()
        .join("profiles")
        .join(sanitize_profile_name(name))
}

pub fn delete_profile(name: &str) {
    let mut file = read_profiles_file();
    file.profiles.retain(|p| p.name != name);
    if file.active.as_deref() == Some(name) {
        file.active = None;
    }
    write_profiles_file(&file);
}

fn ensure_profile_pgn_dir(name: &str) {
    std::fs::create_dir_all(profile_pgn_dir(name)).ok();
}

// ── Log directory ────────────────────────────────────────────────────────────
//
// The game's runtime logs (game.log.<date>, crash_*.log, recovered_errors.log)
// live inside the active profile's folder — the folder the player picked when
// they created the profile, or the per-profile Documents/xfchess/profiles/<name>
// default — so everything about a player (node key, PGNs, logs) stays in one
// discoverable place they can zip up and send. Fresh installs with no profile
// yet fall back to the app's per-user data dir until onboarding picks one.
//
// The directory is resolved once per process and cached: the tracing file
// appender is created very early in startup (LogPlugin), before a profile can
// exist, and it can't be re-pointed mid-session — so a profile created or
// activated during the current session takes effect on the next launch.

static LOG_DIR_CACHE: OnceLock<PathBuf> = OnceLock::new();

pub fn log_dir() -> PathBuf {
    LOG_DIR_CACHE
        .get_or_init(|| {
            let dir = compute_log_dir();
            std::fs::create_dir_all(&dir).ok();
            dir
        })
        .clone()
}

fn compute_log_dir() -> PathBuf {
    load_active_profile()
        .map(|name| profile_pgn_dir(&name).join("logs"))
        .unwrap_or_else(|| fallback_log_dir())
}

#[cfg(not(target_os = "android"))]
fn fallback_log_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess")
        .join("logs")
}

#[cfg(target_os = "android")]
fn fallback_log_dir() -> PathBuf {
    crate::core::paths::internal_data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs")
}

/// The folder a player would think of as "my XFChess folder": the active
/// profile's folder, or the Documents/xfchess default before one exists.
/// Used as the starting point for the Export Logs save dialog so the bundle
/// lands next to everything else the player cares about.
pub fn active_profile_dir() -> PathBuf {
    load_active_profile()
        .map(|name| profile_pgn_dir(&name))
        .unwrap_or_else(|| {
            // Same base profiles_dir() uses; the dialog just needs a plausible,
            // writable starting point.
            #[cfg(not(target_os = "android"))]
            let base = dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("xfchess");
            #[cfg(target_os = "android")]
            let base = crate::core::paths::external_data_dir()
                .unwrap_or_else(|| PathBuf::from("."));
            std::fs::create_dir_all(&base).ok();
            base
        })
}
