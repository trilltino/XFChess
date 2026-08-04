// Regression guard for docs/PRE_MAINNET_E2E_PLAN.md §4.2: `iroh-gossip`'s own
// Cargo.toml used to declare a loose `iroh = "1"` instead of inheriting the
// workspace's exact `=1.0.3` pin, so a `cargo update` inside that one crate
// could silently drift it away from what the rest of the workspace resolves
// to (e.g. mid-game gossip re-sync breaking against a newer/older `iroh`).
// This parses `Cargo.lock` directly (no `cargo` subprocess) so it's fast and
// has no network dependency.

use std::fs;

fn cargo_lock_versions(package_name: &str) -> Vec<String> {
    let lock_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock");
    let contents = fs::read_to_string(lock_path).expect("workspace Cargo.lock must exist");

    let mut versions = Vec::new();
    let mut in_target_package = false;
    for line in contents.lines() {
        if line == "[[package]]" {
            in_target_package = false;
            continue;
        }
        if let Some(name) = line.strip_prefix("name = \"").and_then(|s| s.strip_suffix('"')) {
            in_target_package = name == package_name;
            continue;
        }
        if in_target_package {
            if let Some(version) = line.strip_prefix("version = \"").and_then(|s| s.strip_suffix('"')) {
                versions.push(version.to_string());
            }
        }
    }
    versions
}

#[test]
fn exactly_one_iroh_version_resolves_workspace_wide() {
    let versions = cargo_lock_versions("iroh");
    assert_eq!(
        versions.len(),
        1,
        "expected exactly one `iroh` version in Cargo.lock, found {:?} — a crate (e.g. \
         iroh-gossip) has drifted from the workspace's exact `=1.0.3` pin",
        versions
    );
    assert_eq!(versions[0], "1.0.3");
}

#[test]
fn exactly_one_iroh_base_version_resolves_workspace_wide() {
    let versions = cargo_lock_versions("iroh-base");
    assert_eq!(
        versions.len(),
        1,
        "expected exactly one `iroh-base` version in Cargo.lock, found {:?}",
        versions
    );
    assert_eq!(versions[0], "1.0.3");
}
