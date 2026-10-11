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

/// The version the client reports and compares against GitHub releases: the
/// release tag CI passes in, else the newest `v*` tag this checkout descends
/// from (so local builds don't claim the never-bumped crate version), else the
/// crate version.
fn stamp_version() {
    println!("cargo:rerun-if-env-changed=XFCHESS_RELEASE_VERSION");
    // Re-stamp after a checkout or a new tag.
    for git_path in [".git/HEAD", ".git/refs/tags", ".git/packed-refs"] {
        if Path::new(git_path).exists() {
            println!("cargo:rerun-if-changed={git_path}");
        }
    }
    let version = std::env::var("XFCHESS_RELEASE_VERSION")
        .ok()
        .or_else(latest_release_tag)
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=XFCHESS_VERSION={}", version);
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
