use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    stamp_version()?;
    Ok(())
}

/// The version the client reports and compares against GitHub releases: the
/// release tag CI passes in, else the newest `v*` tag this checkout descends
/// from (so local builds don't claim the never-bumped crate version), else the
/// crate version.
///
/// This must be stamped on the crate that reads it (`core::update_check`):
/// `cargo:rustc-env` from another crate's build script never reaches it.
fn stamp_version() -> Result<(), std::env::VarError> {
    println!("cargo:rerun-if-env-changed=XFCHESS_RELEASE_VERSION");
    // Re-stamp after a checkout or a new tag.
    for git_path in [
        "../../.git/HEAD",
        "../../.git/refs/tags",
        "../../.git/packed-refs",
    ] {
        if Path::new(git_path).exists() {
            println!("cargo:rerun-if-changed={git_path}");
        }
    }
    let version = std::env::var("XFCHESS_RELEASE_VERSION")
        .ok()
        .or_else(latest_release_tag)
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .filter(|v| !v.is_empty());
    let version = match version {
        Some(version) => version,
        None => std::env::var("CARGO_PKG_VERSION")?,
    };
    println!("cargo:rustc-env=XFCHESS_VERSION={}", version);
    Ok(())
}

fn latest_release_tag() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0", "--match", "v[0-9]*"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
