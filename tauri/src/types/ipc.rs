#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcCommand {
  Window(WindowCommand),
  Auth(AuthCommand),
  Config(ConfigCommand),
  System(SystemCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowCommand {
  Show,
  Hide,
  SetTitle { title: String },
  SetSize { width: f64, height: f64 },
  SetPosition { x: f64, y: f64 },
  Minimize,
  Maximize,
  Close,
  SetAlwaysOnTop { always_on_top: bool },
  SetDecorations { decorated: bool },
  SetVisible { visible: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthCommand {
  Login { token: String, backend_url: String },
  Logout,
  ValidateToken,
  GetSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigCommand {
  GetConfig,
  SetBackendUrl { url: String },
  SetAdminApiKey { key: String },
  SetWalletPort { port: u16 },
  GetBackendUrl,
  GetAdminApiKey,
  GetWalletPort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemCommand {
  GetVersion,
  GetPlatform,
  OpenUrl {
    url: String,
  },
  CopyToClipboard {
    text: String,
  },
  ShowNotification {
    title: String,
    body: String,
    level: String,
  },
  Restart,
  Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse<T> {
  pub success: bool,
  pub data: Option<T>,
  pub error: Option<String>,
}

impl<T> IpcResponse<T> {
  pub fn success(data: T) -> Self {
    Self {
      success: true,
      data: Some(data),
      error: None,
    }
  }

  pub fn error(error: String) -> Self {
    Self {
      success: false,
      data: None,
      error: Some(error),
    }
  }
}
