use std::fs;
use std::path::Path;

fn main() {
    stamp_version();

    // Copy assets to target directory for Bevy to find them
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let target_dir = out_dir.ancestors().nth(3).unwrap(); // Go up to target/debug or target/release
    let assets_dest = target_dir.join("assets");

    // Create assets directory if it doesn't exist
    let assets_src = Path::new("assets");
    if assets_src.exists() {
        println!("cargo:rerun-if-changed=assets");
        if let Err(e) = copy_dir_recursive(assets_src, &assets_dest) {
            eprintln!("Warning: Failed to copy assets: {}", e);
        }
    }
}

/// Bake the shipped release version into the binary as `XFCHESS_VERSION`.
///
/// `Cargo.toml`'s version stays at 0.1.0 — releases are cut from git tags
/// (`.github/workflows/release.yml` derives the installer version from
/// `GITHUB_REF_NAME`), so `CARGO_PKG_VERSION` does not track what users
/// actually downloaded. CI passes the tag through `XFCHESS_RELEASE_VERSION`
/// so the in-game update checker can compare this build against the latest
/// GitHub release; local and dev builds fall back to the manifest version.
fn stamp_version() {
    println!("cargo:rerun-if-env-changed=XFCHESS_RELEASE_VERSION");
    let version = std::env::var("XFCHESS_RELEASE_VERSION")
        .ok()
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=XFCHESS_VERSION={}", version);
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
