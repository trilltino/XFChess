// Keep iroh-gossip aligned with the workspace pin. Read Cargo.lock directly
// without invoking Cargo or requiring the network.

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
