//! Platform-neutral host capabilities and request contracts.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletIntegration {
    TauriBridge,
    MobileWalletAdapterPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub wallet_integration: WalletIntegration,
    pub file_dialogs: bool,
    pub self_update: bool,
}

#[derive(Debug, Clone)]
pub struct PlatformPaths {
    pub config: Option<PathBuf>,
    pub app_config: Option<PathBuf>,
    pub local_data: Option<PathBuf>,
    pub data: Option<PathBuf>,
    pub documents: Option<PathBuf>,
    pub global_session_data: Option<PathBuf>,
    pub native_libraries: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSignRequest {
    pub request_id: u64,
    pub transaction: Vec<u8>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSignResponse {
    pub request_id: u64,
    pub result: Result<Vec<u8>, String>,
}
