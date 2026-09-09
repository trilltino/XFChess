#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
  pub token: String,
  pub backend_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
  pub success: bool,
  pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
  pub token: String,
  pub backend_url: String,
  pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
  pub title: String,
  pub visible: bool,
  pub focused: bool,
  pub size: WindowSize,
  pub position: WindowPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
  pub width: f64,
  pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPosition {
  pub x: f64,
  pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
  pub title: String,
  pub body: String,
  pub level: String,
}
