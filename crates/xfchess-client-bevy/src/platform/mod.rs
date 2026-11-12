//! Host capabilities shared by the Bevy client.
//!
//! Platform-specific APIs belong here. Feature code consumes these paths and
//! capabilities instead of selecting Android or desktop implementations itself.

use bevy::prelude::*;
use std::sync::OnceLock;
pub use xfchess_platform_api::{
    PlatformCapabilities, PlatformPaths, WalletIntegration, WalletSignRequest, WalletSignResponse,
};

#[cfg(target_os = "android")]
pub mod android;
#[cfg(not(target_os = "android"))]
mod desktop;

static PATHS: OnceLock<PlatformPaths> = OnceLock::new();

pub fn paths() -> &'static PlatformPaths {
    PATHS.get_or_init(imp::detect_paths)
}

pub fn capabilities() -> PlatformCapabilities {
    imp::capabilities()
}

#[cfg(target_os = "android")]
use android as imp;
#[cfg(not(target_os = "android"))]
use desktop as imp;

pub struct PlatformPlugin;

#[derive(Resource, Debug, Clone)]
pub struct HostPaths(pub PlatformPaths);

#[derive(Resource, Debug, Clone, Copy)]
pub struct HostCapabilities(pub PlatformCapabilities);

impl Plugin for PlatformPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HostPaths(paths().clone()))
            .insert_resource(HostCapabilities(capabilities()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_plugin_exposes_host_contract() {
        let mut app = App::new();
        app.add_plugins(PlatformPlugin);
        assert!(app.world().contains_resource::<HostPaths>());
        assert!(app.world().contains_resource::<HostCapabilities>());
    }
}
