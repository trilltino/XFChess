// The one-click "send us the logs" path: Settings → Help & Support →
// "Export logs as text file". This assembles everything a dev needs to debug
// a reported game — version/OS/arch header, today's (and yesterday's) game
// log, crash reports, recovered errors, and the wallet bridge's log — into a
// single plain-text file the player picks the location for and can drag into
// a group chat. Nothing is uploaded anywhere; the player sees exactly what
// they're sending.
//
// Desktop (Windows/macOS/Linux/Chrome OS) only — Android has no rfd backend,
// so the Settings button is cfg-gated away and this module mostly stays as
// dead code there.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;

// How many lines we keep from the tail of each daily log. Enough to cover a
// full session's worth of moves/subscriptions/errors without letting a
// support bundle balloon to megabytes.
const LOG_TAIL_LINES: usize = 4000;

// How many newest daily files (game.log.YYYY-MM-DD) to include.
const NEWEST_DAILY_FILES: usize = 2;

/// UI state for the Settings-screen export button: the last success/error
/// message, displayed under the button so the player has a written record of
/// where the file went.
#[derive(Resource, Default)]
pub struct SupportBundleUi {
    pub status: Option<String>,
    pub error: Option<String>,
}

fn unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn read_if_exists(path: &PathBuf) -> Option<String> {
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let total = text.lines().count();
    let skip = if total > max_lines {
        total - max_lines
    } else {
        0
    };
    let mut seen = 0;
    let mut out = String::new();
    for line in text.lines() {
        seen += 1;
        if seen > skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn list_files(dir: &PathBuf, pred: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(iter) => iter,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in entries {
        let Some(entry) = entry.ok() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if pred(&name) {
            out.push(entry.path());
        }
    }
    out
}

/// Newest `n` files from a lexicographically sorted (ascending) list — daily
/// names like game.log.2026-09-12 sort chronologically, so this keeps today
/// and yesterday.
fn newest_n(files: &mut Vec<PathBuf>, n: usize) -> Vec<PathBuf> {
    if files.len() <= n {
        return files.clone();
    }
    let start = files.len() - n;
    files[start..].to_vec()
}

/// Appends every crash report + recovered-errors file, plus (optionally) the
/// newest daily game/bridge logs, from `dir`, each snip-cut to a bounded tail.
fn append_dir_sections(mut out: &mut String, dir: &PathBuf, include_game: bool) {
    let mut ordered: Vec<PathBuf> = Vec::new();

    if include_game {
        let mut game_logs = list_files(dir, |n| n.starts_with("game.log."));
        game_logs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for path in newest_n(&mut game_logs, NEWEST_DAILY_FILES) {
            ordered.push(path);
        }
    }

    let mut bridge_logs = list_files(dir, |n| n.starts_with("wallet-bridge.log."));
    bridge_logs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for path in newest_n(&mut bridge_logs, NEWEST_DAILY_FILES) {
        ordered.push(path);
    }

    let mut crashes = list_files(dir, |n| n.starts_with("crash_"));
    crashes.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for path in crashes {
        ordered.push(path);
    }

    for path in ordered {
        let Some(text) = read_if_exists(&path) else {
            continue;
        };
        let body = tail_lines(&text, LOG_TAIL_LINES);
        let title = path.display().to_string();
        out.push('\n');
        out.push_str(&title);
        out.push_str("\n");
        out.push_str(&"=".repeat(title.len()));
        out.push('\n');
        out.push_str(&body);
        out.push('\n');
    }

    let recovered = dir.join("recovered_errors.log");
    if let Some(text) = read_if_exists(&recovered) {
        let title = recovered.display().to_string();
        out.push('\n');
        out.push_str(&title);
        out.push_str("\n");
        out.push_str(&"=".repeat(title.len()));
        out.push('\n');
        out.push_str(&text);
        out.push('\n');
    }
}
fn build_header(log_dir: &PathBuf) -> String {
    #[cfg(debug_assertions)]
    let build = "debug";
    #[cfg(not(debug_assertions))]
    let build = "release";

    let profile = crate::multiplayer::network::identity::load_active_profile()
        .unwrap_or_else(|| "(none)".to_string());

    let mut out = String::new();
    out.push_str("XFChess support bundle\n");
    out.push_str("======================\n");
    out.push_str(&format!("Generated (unix seconds): {}\n", unix_ts()));
    out.push_str(&format!("Version: {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!(
        "OS: {}  |  Arch: {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    out.push_str(&format!("Build: {}\n", build));
    out.push_str(&format!("Profile: {}\n", profile));
    out.push_str(&format!("Log folder: {}\n", log_dir.display()));
    out.push_str(
        "======================\n\
         Search for [GAME-START] and [GAME-END] lines to cut around a single\n\
         reported game — online games carry their numeric game_id there.\n",
    );
    out
}

/// Writes the support bundle to `dest`. Returns a player-facing error message
/// on failure (e.g. a non-writable pick).
pub fn write_support_bundle(dest: &PathBuf) -> Result<(), String> {
    let log_dir = crate::multiplayer::network::identity::log_dir();

    // The wallet bridge is a separate process that still writes to the
    // app-data base folder; fold it in when that differs from the active
    // profile's log folder (it's the same folder on fresh installs).
    #[cfg(not(target_os = "android"))]
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xfchess")
        .join("logs");
    #[cfg(target_os = "android")]
    let base_dir = log_dir.clone();

    let mut out = String::new();
    out.push_str(&build_header(&log_dir));

    if base_dir == log_dir {
        append_dir_sections(&mut out, &log_dir, true);
    } else {
        // Game + crash logs from the profile folder, wallet-bridge from base.
        append_dir_sections(&mut out, &log_dir, true);
        append_dir_sections(&mut out, &base_dir, false);
    }

    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dest)
    else {
        return Err(format!(
            "Could not create {} — pick a writable folder.",
            dest.display()
        ));
    };
    let _ = writeln!(file, "{}", out);
    Ok(())
}

/// Desktop only: pops the OS save dialog (defaulting to the active profile's
/// folder), writes the bundle there, and records the result for the Settings
/// screen to display. Android never calls this — the button is cfg-gated off.
#[cfg(not(target_os = "android"))]
pub fn run_export_dialog(mut ui: ResMut<SupportBundleUi>) {
    let default_dir = crate::multiplayer::network::identity::active_profile_dir();
    let picked = rfd::FileDialog::new()
        .set_title("Export XFChess Logs")
        .set_file_name(format!("xfchess-logs-{}.txt", unix_ts()))
        .set_directory(&default_dir)
        .save_file();
    let Some(path) = picked else { return };
    match write_support_bundle(&path) {
        Ok(()) => {
            ui.status = Some(format!("Saved to {}", path.display()));
            info!("[support] Support bundle saved to {}", path.display());
        }
        Err(e) => {
            ui.error = Some(e.clone());
            warn!("[support] Support bundle export failed: {e}");
        }
    }
}
