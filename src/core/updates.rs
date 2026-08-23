//! In-game update checker.
//!
//! Compares this build's stamped version (`XFCHESS_VERSION`, baked in by
//! `build.rs` from the release tag) against the newest published GitHub
//! release. When a newer one exists the main menu surfaces a download panel
//! listing every supported OS, and each button opens that platform's release
//! asset directly.
//!
//! The client never installs anything itself — it hands the download to the
//! user's browser, the same path `xfchess.com`'s download page takes (see
//! `xfchessdotcom/src/pages/Play.tsx`, whose asset patterns this mirrors).

use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use crossbeam_channel::{unbounded, Receiver, TryRecvError};
use std::path::PathBuf;
use std::time::Duration;

/// The version this build shipped as — the release tag on CI builds, the
/// `Cargo.toml` version locally. See `stamp_version` in `build.rs`.
pub const CURRENT_VERSION: &str = env!("XFCHESS_VERSION");

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/trilltino/XFChess/releases/latest";

/// Fallback target whenever a per-platform asset can't be resolved.
pub const RELEASES_URL: &str = "https://github.com/trilltino/XFChess/releases";
pub const INSTALL_GUIDE_URL: &str =
    "https://github.com/trilltino/XFChess/blob/main/docs/INSTALL.md";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// ─────────────────────────────── Platforms ───────────────────────────────

/// A platform XFChess publishes a release asset for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
    ChromeOs,
}

impl Platform {
    /// Display order in the download panel.
    pub const ALL: [Platform; 4] = [
        Platform::Windows,
        Platform::MacOs,
        Platform::Linux,
        Platform::ChromeOs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Platform::Windows => "Windows",
            Platform::MacOs => "macOS",
            Platform::Linux => "Linux",
            Platform::ChromeOs => "Chrome OS",
        }
    }

    /// What the download actually is, shown under the label.
    pub fn kind(self) -> &'static str {
        match self {
            Platform::Windows => "Installer (.exe)",
            Platform::MacOs => "Disk image (.dmg)",
            Platform::Linux => "Archive (.tar.gz)",
            Platform::ChromeOs => "Crostini archive (.tar.gz)",
        }
    }

    /// Icon rasterised from the marks the website uses (see
    /// `xfchessdotcom/src/components/PlatformIcons.tsx`), except Chrome OS —
    /// the website has no Chrome OS card, so its mark comes straight from
    /// simple-icons' `googlechrome.svg` (CC0), the same source the Apple and
    /// Linux marks are drawn from. All four are white-on-transparent so they
    /// read as one set on the dark download card.
    pub fn icon_path(self) -> &'static str {
        match self {
            Platform::Windows => "assets/branding/platforms/windows.png",
            Platform::MacOs => "assets/branding/platforms/macos.png",
            Platform::Linux => "assets/branding/platforms/linux.png",
            // The Chrome OS release *is* the Linux build, relabelled by the
            // `chromeos` job in release.yml, but it gets the Chrome mark
            // rather than Tux: the card is how a Chrome OS user identifies
            // their own download, and a penguin doesn't say "this one".
            Platform::ChromeOs => "assets/branding/platforms/chromeos.png",
        }
    }

    /// Whether `name` is this platform's release asset.
    ///
    /// Mirrors `ASSET_PATTERNS` in `xfchessdotcom/src/pages/Play.tsx` and the
    /// filenames produced by `.github/workflows/release.yml`.
    fn matches_asset(self, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        match self {
            Platform::Windows => n.starts_with("xfchess-setup-") && n.ends_with(".exe"),
            Platform::MacOs => n.starts_with("xfchess-") && n.ends_with(".dmg"),
            Platform::Linux => n.starts_with("xfchess-linux-") && n.ends_with(".tar.gz"),
            Platform::ChromeOs => n.starts_with("xfchess-chromeos-") && n.ends_with(".tar.gz"),
        }
    }

    /// The platform this build is running on, if XFChess publishes for it.
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "windows") {
            Some(Platform::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Platform::MacOs)
        } else if cfg!(target_os = "linux") {
            Some(if running_on_chrome_os() {
                Platform::ChromeOs
            } else {
                Platform::Linux
            })
        } else {
            None
        }
    }
}

/// Detect a Chrome OS (Crostini) container.
///
/// Crostini VMs expose the host's milestone at `/dev/.cros_milestone`, and
/// every Crostini GUI app runs through sommelier, which advertises itself in
/// the environment. Either marker is enough, and a wrong answer is cosmetic:
/// both platforms are served the identical tarball under different names.
#[cfg(target_os = "linux")]
fn running_on_chrome_os() -> bool {
    std::path::Path::new("/dev/.cros_milestone").exists()
        || std::env::var_os("SOMMELIER_VERSION").is_some()
}

#[cfg(not(target_os = "linux"))]
fn running_on_chrome_os() -> bool {
    false
}

// ───────────────────────────── Release data ──────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

impl ReleaseAsset {
    /// Human-readable size. Empty when GitHub reported none.
    pub fn size_label(&self) -> String {
        if self.size == 0 {
            return String::new();
        }
        let mb = self.size as f64 / 1_048_576.0;
        if mb >= 1024.0 {
            format!("{:.1} GB", mb / 1024.0)
        } else {
            format!("{mb:.0} MB")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// Tag with any leading `v` stripped, e.g. "0.0.1".
    pub version: String,
    pub tag: String,
    pub page_url: String,
    /// ISO-8601 publish timestamp, as GitHub reported it.
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

impl ReleaseInfo {
    pub fn asset_for(&self, platform: Platform) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|a| platform.matches_asset(&a.name))
    }

    /// Direct asset link when one exists, otherwise the release page — so a
    /// button is never dead, even mid-release while assets are still uploading.
    pub fn download_url_for(&self, platform: Platform) -> &str {
        self.asset_for(platform)
            .map(|a| a.url.as_str())
            .unwrap_or(&self.page_url)
    }

    /// Just the `YYYY-MM-DD` part of the publish timestamp.
    pub fn published_date(&self) -> Option<&str> {
        self.published_at.as_deref().and_then(|s| s.get(..10))
    }
}

/// GitHub's release payload — only the fields this checker reads.
#[derive(serde::Deserialize)]
struct WireRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<WireAsset>,
}

#[derive(serde::Deserialize)]
struct WireAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

// ────────────────────────── Version comparison ───────────────────────────

/// A dotted numeric version, tolerant of a leading `v` and of any
/// `-prerelease` / `+build` suffix.
///
/// Field order matters: derived `Ord` compares `parts` first, then `release`,
/// so `1.0.0-rc.1` (false) sorts below `1.0.0` (true), as semver requires.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Version {
    parts: [u64; 3],
    release: bool,
}

fn parse_version(raw: &str) -> Option<Version> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?;
    if core.is_empty() {
        return None;
    }
    let mut parts = [0u64; 3];
    for (i, segment) in core.split('.').take(3).enumerate() {
        parts[i] = segment.parse().ok()?;
    }
    Some(Version {
        parts,
        release: core.len() == trimmed.len(),
    })
}

/// Is `latest` a newer version than `current`?
///
/// Anything unparseable answers "no": never nag the user over a version
/// string we don't understand.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

// ──────────────────────────── Checker state ──────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    /// No check has run yet.
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    /// Check failed; holds a short reason for the UI.
    Failed(String),
}

#[derive(Resource)]
pub struct UpdateCheck {
    pub status: UpdateStatus,
    /// Whether the download panel is on screen.
    pub panel_open: bool,
    /// Version the user pressed "Skip this version" on — remembered across
    /// launches so a declined update never re-opens the panel by itself.
    skipped: Option<String>,
    rx: Option<Receiver<Result<ReleaseInfo, String>>>,
}

impl Default for UpdateCheck {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            panel_open: false,
            skipped: load_skipped_version(),
            rx: None,
        }
    }
}

impl UpdateCheck {
    pub fn available(&self) -> Option<&ReleaseInfo> {
        match &self.status {
            UpdateStatus::Available(release) => Some(release),
            _ => None,
        }
    }

    pub fn is_checking(&self) -> bool {
        matches!(self.status, UpdateStatus::Checking)
    }

    /// Start a check, unless one is already in flight.
    pub fn start(&mut self) {
        if self.is_checking() {
            return;
        }
        let (tx, rx) = unbounded();
        self.rx = Some(rx);
        self.status = UpdateStatus::Checking;

        // `reqwest::blocking` is safe here: `IoTaskPool` is Bevy's own pool
        // for exactly this kind of blocking IO, not a tokio runtime (building
        // a blocking client inside one of those panics).
        IoTaskPool::get()
            .spawn(async move {
                let _ = tx.send(fetch_latest_release());
            })
            .detach();
    }

    /// Remember the offered version as declined, and close the panel.
    pub fn skip_available_version(&mut self) {
        if let Some(release) = self.available() {
            let version = release.version.clone();
            save_skipped_version(&version);
            self.skipped = Some(version);
        }
        self.panel_open = false;
    }
}

// ───────────────────────────── Networking ────────────────────────────────

fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        // GitHub rejects API requests that arrive without a User-Agent.
        .user_agent(concat!("XFChess/", env!("XFCHESS_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let response = client
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|e| format!("Could not reach GitHub ({e})"))?;

    let status = response.status();
    if !status.is_success() {
        // 403 here is almost always the 60-requests/hour unauthenticated rate
        // limit, which is shared per-IP.
        return Err(format!("GitHub returned {status}"));
    }

    let wire: WireRelease = response
        .json()
        .map_err(|e| format!("Unreadable release feed ({e})"))?;

    Ok(ReleaseInfo {
        version: wire
            .tag_name
            .trim()
            .trim_start_matches(['v', 'V'])
            .to_string(),
        tag: wire.tag_name,
        page_url: if wire.html_url.is_empty() {
            RELEASES_URL.to_string()
        } else {
            wire.html_url
        },
        published_at: wire.published_at,
        assets: wire
            .assets
            .into_iter()
            .map(|a| ReleaseAsset {
                name: a.name,
                url: a.browser_download_url,
                size: a.size,
            })
            .collect(),
    })
}

/// Open `url` in the user's browser. GitHub serves release assets with
/// `Content-Disposition: attachment`, so an asset link downloads the file
/// rather than navigating anywhere.
pub fn open_in_browser(url: &str) {
    match webbrowser::open(url) {
        Ok(()) => info!("[UPDATE] Opened {url}"),
        Err(e) => warn!("[UPDATE] Could not open browser for {url}: {e}"),
    }
}

// ─────────────────────── Skipped-version persistence ─────────────────────

/// Sits next to `settings.json` (see `core::settings_persistence`).
const SKIP_FILENAME: &str = "update_check.json";

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct SkipState {
    skipped_version: Option<String>,
}

fn skip_file_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "trilltino", "XFChess")
        .map(|dirs| dirs.config_dir().join(SKIP_FILENAME))
}

fn load_skipped_version() -> Option<String> {
    let path = skip_file_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<SkipState>(&contents)
        .ok()?
        .skipped_version
}

fn save_skipped_version(version: &str) {
    let Some(path) = skip_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("[UPDATE] Could not create config dir: {e}");
            return;
        }
    }
    let state = SkipState {
        skipped_version: Some(version.to_string()),
    };
    match serde_json::to_string_pretty(&state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("[UPDATE] Could not save skipped version: {e}");
            }
        }
        Err(e) => warn!("[UPDATE] Could not serialise skipped version: {e}"),
    }
}

// ─────────────────────────────── Systems ─────────────────────────────────

fn start_update_check(mut check: ResMut<UpdateCheck>) {
    info!("[UPDATE] Running {CURRENT_VERSION}; checking for a newer release");
    check.start();
}

/// Pick up the worker's answer and decide whether to raise the panel.
fn poll_update_check(mut check: ResMut<UpdateCheck>) {
    let Some(rx) = check.rx.as_ref() else {
        return;
    };
    let result = match rx.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err("Update check stopped unexpectedly".to_string()),
    };
    check.rx = None;

    match result {
        Ok(release) if is_newer(&release.version, CURRENT_VERSION) => {
            info!(
                "[UPDATE] {} is available (running {CURRENT_VERSION})",
                release.version
            );
            // Auto-raise once per version; a version the user skipped stays
            // quiet until they open the panel from the menu themselves.
            check.panel_open = check.skipped.as_deref() != Some(release.version.as_str());
            check.status = UpdateStatus::Available(release);
        }
        Ok(release) => {
            info!(
                "[UPDATE] Up to date ({CURRENT_VERSION}; latest release is {})",
                release.version
            );
            check.status = UpdateStatus::UpToDate;
        }
        Err(e) => {
            warn!("[UPDATE] Check failed: {e}");
            check.status = UpdateStatus::Failed(e);
        }
    }
}

pub struct UpdateCheckPlugin;

impl Plugin for UpdateCheckPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UpdateCheck>()
            .add_systems(Startup, start_update_check)
            .add_systems(Update, poll_update_check);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer("0.0.2", "0.0.1"));
        assert!(is_newer("0.1.0", "0.0.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.2.10", "0.2.9"));
    }

    #[test]
    fn same_or_older_versions_are_not_offered() {
        assert!(!is_newer("0.0.1", "0.0.1"));
        assert!(!is_newer("0.0.1", "0.0.2"));
        // A dev build ahead of the newest release must not be told to
        // "update" backwards.
        assert!(!is_newer("0.0.1", "0.1.0"));
    }

    #[test]
    fn tags_may_carry_a_v_prefix() {
        assert!(is_newer("v0.0.2", "0.0.1"));
        assert!(!is_newer("v0.0.1", "v0.0.1"));
    }

    #[test]
    fn prereleases_sort_below_their_release() {
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0"));
        assert!(is_newer("1.0.0-rc.1", "0.9.0"));
    }

    #[test]
    fn unparseable_versions_never_nag() {
        assert!(!is_newer("nightly", "0.0.1"));
        assert!(!is_newer("0.0.2", "unknown"));
        assert!(!is_newer("", "0.0.1"));
    }

    #[test]
    fn assets_match_the_release_workflow_filenames() {
        let release = ReleaseInfo {
            version: "0.0.1".into(),
            tag: "v0.0.1".into(),
            page_url: "https://github.com/trilltino/XFChess/releases/tag/v0.0.1".into(),
            published_at: Some("2026-08-16T21:28:24Z".into()),
            assets: vec![
                asset("XFChess-0.0.1.dmg"),
                asset("XFChess-linux-x86_64-0.0.1.tar.gz"),
                asset("XFChess-chromeos-x86_64-0.0.1.tar.gz"),
                asset("XFChess-Setup-0.0.1.exe"),
            ],
        };

        let name = |p: Platform| release.asset_for(p).map(|a| a.name.as_str());
        assert_eq!(name(Platform::Windows), Some("XFChess-Setup-0.0.1.exe"));
        assert_eq!(name(Platform::MacOs), Some("XFChess-0.0.1.dmg"));
        assert_eq!(
            name(Platform::Linux),
            Some("XFChess-linux-x86_64-0.0.1.tar.gz")
        );
        assert_eq!(
            name(Platform::ChromeOs),
            Some("XFChess-chromeos-x86_64-0.0.1.tar.gz")
        );
    }

    #[test]
    fn a_missing_asset_falls_back_to_the_release_page() {
        // v0.0.1 shipped without a Chrome OS asset.
        let release = ReleaseInfo {
            version: "0.0.1".into(),
            tag: "v0.0.1".into(),
            page_url: "https://github.com/trilltino/XFChess/releases/tag/v0.0.1".into(),
            published_at: None,
            assets: vec![asset("XFChess-Setup-0.0.1.exe")],
        };
        assert_eq!(release.asset_for(Platform::ChromeOs), None);
        assert_eq!(
            release.download_url_for(Platform::ChromeOs),
            "https://github.com/trilltino/XFChess/releases/tag/v0.0.1"
        );
    }

    #[test]
    fn published_date_is_the_calendar_day() {
        let release = ReleaseInfo {
            version: "0.0.1".into(),
            tag: "v0.0.1".into(),
            page_url: RELEASES_URL.into(),
            published_at: Some("2026-08-16T21:28:24Z".into()),
            assets: vec![],
        };
        assert_eq!(release.published_date(), Some("2026-08-16"));
    }

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            url: format!("https://example.invalid/{name}"),
            size: 150 * 1_048_576,
        }
    }
}
