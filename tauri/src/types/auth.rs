//! Unused auth/window/notification DTOs (`#![allow(dead_code)]` below is
//! deliberate — nothing in `services::ipc`'s command handlers constructs or
//! reads these). The real wallet/JWT state the bridge actually uses is
//! `WalletPubkey`/`WalletJwt` in `main.rs`, not `AuthRequest`/`AuthResponse`/
//! `UserSession` here.

#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
  pub token: String,
  pub backend_url: String,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
  pub success: bool,
  pub message: Option<String>,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
  pub token: String,
  pub backend_url: String,
  pub expires_at: Option<u64>,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
  pub title: String,
  pub visible: bool,
  pub focused: bool,
  pub size: WindowSize,
  pub position: WindowPosition,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
  pub width: f64,
  pub height: f64,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPosition {
  pub x: f64,
  pub y: f64,
}

/// Unused. See module docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
  pub title: String,
  pub body: String,
  pub level: String,
}
