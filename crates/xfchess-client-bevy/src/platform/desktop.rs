#![cfg(not(target_os = "android"))]

use super::{PlatformCapabilities, PlatformPaths, WalletIntegration};

pub(super) fn detect_paths() -> PlatformPaths {
    let project = directories::ProjectDirs::from("com", "trilltino", "XFChess");
    let global_session = directories::ProjectDirs::from("com", "xfchess", "XFChess");
    PlatformPaths {
        config: dirs::config_dir().map(|path| path.join("xfchess")),
        app_config: project.as_ref().map(|dirs| dirs.config_dir().to_path_buf()),
        local_data: dirs::data_local_dir().map(|path| path.join("xfchess")),
        data: dirs::data_dir().map(|path| path.join("xfchess")),
        documents: dirs::document_dir().map(|path| path.join("xfchess")),
        global_session_data: global_session
            .as_ref()
            .map(|dirs| dirs.data_local_dir().to_path_buf()),
        native_libraries: None,
    }
}

pub(super) fn capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        wallet_integration: WalletIntegration::TauriBridge,
        file_dialogs: true,
        self_update: true,
    }
}
