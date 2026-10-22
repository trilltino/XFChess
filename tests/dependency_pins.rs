// Keep iroh and iroh-base on a single version, the exact one the workspace pins
// (iroh-gossip has pulled in a second copy before). Reads Cargo.toml and
// Cargo.lock directly, without invoking Cargo or the network.

use std::fs;

fn read_workspace_file(name: &str) -> anyhow::Result<String> {
    let path = format!("{}/{name}", env!("CARGO_MANIFEST_DIR"));
    Ok(fs::read_to_string(path)?)
}

/// The exact version pinned under `[workspace.dependencies]` (`name = "=X.Y.Z"`).
fn workspace_pin(package_name: &str) -> anyhow::Result<String> {
    let manifest = read_workspace_file("Cargo.toml")?;
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
            return pin.strip_prefix('=').map(str::to_string).ok_or_else(|| {
                anyhow::anyhow!("`{package_name}` must be an exact `=` pin, got {pin}")
            });
        }
    }
    anyhow::bail!("`{package_name}` is not pinned in [workspace.dependencies]");
}

fn cargo_lock_versions(package_name: &str) -> anyhow::Result<Vec<String>> {
    let contents = read_workspace_file("Cargo.lock")?;

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
    Ok(versions)
}

fn assert_single_pinned_version(package_name: &str) -> anyhow::Result<()> {
    let pin = workspace_pin(package_name)?;
    let versions = cargo_lock_versions(package_name)?;
    assert_eq!(
        versions.as_slice(),
        std::slice::from_ref(&pin),
        "expected exactly one `{package_name}` in Cargo.lock, at the workspace pin \
         `={pin}` — a dependency (e.g. iroh-gossip) has drifted from it"
    );
    Ok(())
}

#[test]
fn exactly_one_iroh_version_resolves_workspace_wide() -> anyhow::Result<()> {
    assert_single_pinned_version("iroh")
}

#[test]
fn exactly_one_iroh_base_version_resolves_workspace_wide() -> anyhow::Result<()> {
    assert_single_pinned_version("iroh-base")
}

fn contract_configuration(manifest: &str) -> Vec<&str> {
    let mut selected = false;
    let mut entries = Vec::new();
    for line in manifest.lines() {
        let line = line
            .split_once('#')
            .map_or(line, |(content, _)| content)
            .trim();
        if line.starts_with('[') {
            selected = matches!(line, "[features]" | "[dependencies]");
        }
        if selected && !line.is_empty() {
            entries.push(line);
        }
    }
    entries.sort_unstable();
    entries
}

#[test]
fn host_and_sbf_contract_configuration_match() -> anyhow::Result<()> {
    let host = read_workspace_file("programs/xfchess-game/Cargo.toml")?;
    let sbf = read_workspace_file("programs/xfchess-game-sbf/Cargo.toml")?;
    assert_eq!(
        contract_configuration(&host),
        contract_configuration(&sbf),
        "Host tests and the deployed contract must use identical features and dependency pins"
    );
    Ok(())
}
