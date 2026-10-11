// Keep iroh and iroh-base on a single version, the exact one the workspace pins
// (iroh-gossip has pulled in a second copy before). Reads Cargo.toml and
// Cargo.lock directly, without invoking Cargo or the network.

use std::fs;

fn read_workspace_file(name: &str) -> String {
    let path = format!("{}/{name}", env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The exact version pinned under `[workspace.dependencies]` (`name = "=X.Y.Z"`).
fn workspace_pin(package_name: &str) -> String {
    let manifest = read_workspace_file("Cargo.toml");
    let mut in_workspace_deps = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_workspace_deps = line == "[workspace.dependencies]";
            continue;
        }
        if !in_workspace_deps {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == package_name {
            let pin = value.trim().trim_matches('"');
            return pin
                .strip_prefix('=')
                .unwrap_or_else(|| panic!("`{package_name}` must be an exact `=` pin, got {pin}"))
                .to_string();
        }
    }
    panic!("`{package_name}` is not pinned in [workspace.dependencies]");
}

fn cargo_lock_versions(package_name: &str) -> Vec<String> {
    let contents = read_workspace_file("Cargo.lock");

    let mut versions = Vec::new();
    let mut in_target_package = false;
    for line in contents.lines() {
        if line == "[[package]]" {
            in_target_package = false;
            continue;
        }
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|s| s.strip_suffix('"'))
        {
            in_target_package = name == package_name;
            continue;
        }
        if in_target_package {
            if let Some(version) = line
                .strip_prefix("version = \"")
                .and_then(|s| s.strip_suffix('"'))
            {
                versions.push(version.to_string());
            }
        }
    }
    versions
}

fn assert_single_pinned_version(package_name: &str) {
    let pin = workspace_pin(package_name);
    let versions = cargo_lock_versions(package_name);
    assert_eq!(
        versions,
        [pin.clone()],
        "expected exactly one `{package_name}` in Cargo.lock, at the workspace pin \
         `={pin}` — a dependency (e.g. iroh-gossip) has drifted from it"
    );
}

#[test]
fn exactly_one_iroh_version_resolves_workspace_wide() {
    assert_single_pinned_version("iroh");
}

#[test]
fn exactly_one_iroh_base_version_resolves_workspace_wide() {
    assert_single_pinned_version("iroh-base");
}
