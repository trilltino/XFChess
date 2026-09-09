#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
  pub backend_url: String,
  pub wallet_port: u16,
  pub admin_api_key: Option<String>,
  pub log_level: String,
  pub development: bool,
}

impl Default for AppConfig {
  fn default() -> Self {
    Self {
      backend_url: "http://127.0.0.1:8090".to_string(),
      wallet_port: 7454,
      admin_api_key: None,
      log_level: "info".to_string(),
      development: cfg!(debug_assertions),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
  pub title: String,
  pub width: f64,
  pub height: f64,
  pub min_width: f64,
  pub min_height: f64,
  pub resizable: bool,
  pub decorations: bool,
  pub center: bool,
  pub visible: bool,
  pub always_on_top: Option<bool>,
  pub transparent: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
  pub csp: ContentSecurityPolicy,
  pub file_drop_enabled: bool,
  pub auto_zoom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSecurityPolicy {
  pub default_src: String,
  pub connect_src: String,
  pub script_src: String,
  pub style_src: String,
  pub img_src: String,
  pub font_src: String,
  pub object_src: String,
  pub media_src: String,
  pub frame_src: String,
}
