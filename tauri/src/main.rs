//! XFChess Tauri Application
//!
//! This is the main entry point for the XFChess desktop application.
//! It initializes the Tauri runtime, sets up window management,
//! and configures IPC communication between frontend and backend.
//!
//! # Architecture
//!
//! - **Multi-window**: Main app, wallet popup, and tournament admin windows
//! - **IPC Communication**: Commands for window control and system integration
//! - **Shared State**: Global state for wallet and authentication
//! - **Deep Links**: Custom URL scheme handling (xfchess://)
//!
//! # Features
//!
//! - `wallet`: Wallet integration functionality
//! - `tournament-admin`: Tournament administration interface
//! - `dev`: Development-specific features
//! - `all`: Enable all features

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::http::{Method, StatusCode};
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

// Module declarations
mod error;
mod services;
mod types;
mod utils;
mod windows;

// Import commonly used items
use error::{AppError, AppResult};
use services::auth::AuthState;
use services::config::{get_admin_api_key, get_wallet_port};
use utils::logging::init_logging;
use windows::tournament_admin::TournamentAdminWindow;
use windows::wallet::WalletWindow;

// ---------------------------------------------------------------------------
// Shared State
// ---------------------------------------------------------------------------

/// Wallet public key in base58 format.
#[allow(dead_code)]
#[derive(Default, Clone)]
struct WalletPubkey(Arc<Mutex<Option<String>>>);

/// Type alias for in-flight signing request.
type PendingTxInner = Option<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, String>>)>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

/// Get the HTTP port for the wallet signing service.
#[allow(dead_code)]
fn http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

/// Redirect authentication pages to the onboarding single-page application.
#[allow(dead_code)]
async fn redirect_to_onboard() -> impl IntoResponse {
  axum::response::Redirect::to("/onboard")
}

/// Open the wallet popup window in the default browser.
#[tauri::command]
fn show_wallet_popup_window(app: tauri::AppHandle) {
  if let Some(window) = app.get_webview_window("wallet-popup") {
    window.show().unwrap();
    window.set_focus().unwrap();
  }
}

/// Open the tournament admin window in the default browser.
#[tauri::command]
fn show_tournament_admin_window(app: tauri::AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.show().unwrap();
    window.set_focus().unwrap();
  }
}

// ---------------------------------------------------------------------------
// Main Application Entry Point
// ---------------------------------------------------------------------------

fn main() {
  // Initialize logging system first to capture all subsequent logs
  init_logging();

  // Build and run Tauri application
  tauri::Builder::default()
    .plugin(tauri_plugin_deep_link::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    .setup(|app| {
      // Initialize shared application state
      let wallet_pubkey = Arc::new(Mutex::new(None::<String>));
      let pending_tx: PendingTx = Arc::new(Mutex::new(None));
      let auth_state = services::auth::AuthState::new();

      // Register shared state with Tauri app
      app.manage(wallet_pubkey);
      app.manage(pending_tx);
      app.manage(auth_state);

      // Initialize windows
      let handle = app.handle();
      
      #[cfg(feature = "tournament-admin")]
      {
        let _ = TournamentAdminWindow::new(handle);
      }
      
      let _ = WalletWindow::new(handle);

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      show_tournament_admin_window,
      show_wallet_popup_window,
      services::ipc::show_tournament_admin,
      services::ipc::hide_tournament_admin,
      services::ipc::set_tournament_admin_title,
      services::ipc::set_tournament_admin_size,
      services::ipc::set_tournament_admin_position,
      services::ipc::minimize_tournament_admin,
      services::ipc::maximize_tournament_admin,
      services::ipc::close_tournament_admin,
      services::ipc::show_notification,
      services::ipc::open_url,
      services::ipc::copy_to_clipboard,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
