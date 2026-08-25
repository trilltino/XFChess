//! Android-only directory primitives, used by the existing per-call-site
//! `dirs`/`directories` logic scattered across the codebase.
//!
//! This module deliberately does **not** provide four unified "the" app
//! directories. The existing desktop call sites already disagree with each
//! other about directory shape — some use `directories::ProjectDirs::from(
//! "com", "xfchess", "XFChess")`, at least one uses the `"trilltino"`
//! qualifier instead, others use plain `dirs::data_local_dir().join("xfchess")`
//! with no `ProjectDirs` two-level structure at all. Routing every site
//! through one shared scheme here would silently move where a returning
//! desktop user's existing data lives — out of scope for an Android port.
//! Instead, each site keeps its own desktop logic byte-for-byte and gains a
//! parallel `#[cfg(target_os = "android")]` branch built on the two
//! primitives below.
//!
//! `dirs`/`directories` both return `None` on Android (no `HOME`-style
//! per-user directory model), and every existing call site's
//! `.unwrap_or_else(|| PathBuf::from("."))` fallback silently resolves to the
//! read-only APK root — writes there fail silently too, so this isn't a
//! loud crash, just data that never persists across launches. That's the bug
//! this module exists to close.

#![cfg(target_os = "android")]

use std::path::PathBuf;

/// Private, app-scoped storage: Android's `internal_data_path()`
/// (`/data/data/<package>/files`). Already unique per package — unlike
/// desktop's shared `dirs::data_local_dir()`, callers do not need to append
/// an `"xfchess"` subfolder to avoid colliding with other apps' data.
///
/// Returns `None` only if called before Bevy's `#[bevy_main]`-generated
/// `android_main` has stashed `AndroidApp` into `bevy::android::ANDROID_APP`
/// — should not happen given the call order in that generated code (see
/// `src/android/mod.rs`), but calling code should still handle it rather
/// than panic, matching how every existing desktop call site already
/// tolerates a `None` from `dirs::*`.
pub fn internal_data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP.get()?.internal_data_path()
}

/// User-associated, still app-scoped storage: Android's `external_data_path()`
/// (`/storage/emulated/0/Android/data/<package>/files`).
///
/// This is the closest Android equivalent to the desktop call sites that use
/// `dirs::document_dir()` — not the literal shared Documents folder (scoped
/// storage means an app cannot write there without the Storage Access
/// Framework, which is out of scope for the paths this module covers), but
/// visible to the user through a file manager and preserved across app
/// updates (though not across uninstall, unlike true external/removable
/// storage).
pub fn external_data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP.get()?.external_data_path()
}
