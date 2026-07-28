//! All `#[tauri::command]` handlers: window management, notifications,
//! clipboard, and URL opening. `WindowCommands`/`IpcCommands` below are the
//! real, live command enums (distinct from the unused, similarly-named types
//! in `types::ipc` — see that module's docs).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

/// Unused — never constructed anywhere.
pub struct IpcServer;

impl IpcServer {
  pub fn new() -> Self {
    Self
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowCommands {
  ShowTournamentAdmin,
  HideTournamentAdmin,
  SetTournamentAdminTitle { title: String },
  SetTournamentAdminSize { width: f64, height: f64 },
  SetTournamentAdminPosition { x: f64, y: f64 },
  MinimizeTournamentAdmin,
  MaximizeTournamentAdmin,
  CloseTournamentAdmin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcCommands {
  GetWindowInfo,
  ShowNotification { title: String, body: String },
  OpenUrl { url: String },
  CopyToClipboard { text: String },
}

#[tauri::command]
pub fn show_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.show().ok();
    window.set_focus().ok();
  }
}

#[tauri::command]
pub fn hide_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.hide().ok();
  }
}

#[tauri::command]
pub fn set_tournament_admin_title(title: String, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.set_title(&title).ok();
  }
}

#[tauri::command]
pub fn set_tournament_admin_size(width: f64, height: f64, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    let size = tauri::Size::Physical(tauri::PhysicalSize {
      width: width as u32,
      height: height as u32,
    });
    window.set_size(size).ok();
  }
}

#[tauri::command]
pub fn set_tournament_admin_position(x: f64, y: f64, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    let pos = tauri::Position::Physical(tauri::PhysicalPosition {
      x: x as i32,
      y: y as i32,
    });
    window.set_position(pos).ok();
  }
}

#[tauri::command]
pub fn minimize_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.minimize().ok();
  }
}

#[tauri::command]
pub fn maximize_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.maximize().ok();
  }
}

/// Flip maximized/restored state — used by the custom (decorations-off) title
/// bar's maximize button and its double-click-to-maximize handler, since a
/// borderless window has no OS-drawn button that already knows how to do this.
#[tauri::command]
pub fn toggle_maximize_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    match window.is_maximized() {
      Ok(true) => {
        window.unmaximize().ok();
      }
      _ => {
        window.maximize().ok();
      }
    }
  }
}

/// Lets the custom title bar draw the right maximize-vs-restore icon on
/// mount, without needing a `core:window:*` ACL grant for the JS window API.
#[tauri::command]
pub fn is_tournament_admin_maximized(app: AppHandle) -> bool {
  app
    .get_webview_window("tournament-admin")
    .and_then(|w| w.is_maximized().ok())
    .unwrap_or(false)
}

#[tauri::command]
pub fn close_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.close().ok();
  }
}

#[tauri::command]
pub fn show_notification(title: String, body: String, app: AppHandle) {
  if let Err(e) = app.notification().builder().title(title).body(body).show() {
    tracing::warn!("[ipc] show_notification failed: {e}");
  }
}

#[tauri::command]
pub fn open_url(url: String, _app: AppHandle) {
  // Only open safe, expected schemes. `open::that` maps to ShellExecute on Windows,
  // which would otherwise launch arbitrary files/executables/protocol handlers.
  const ALLOWED: [&str; 4] = ["http://", "https://", "mailto:", "xfchess://"];
  if ALLOWED.iter().any(|p| url.starts_with(p)) {
    if let Err(e) = open::that(&url) {
      tracing::warn!("[ipc] open_url failed: {e}");
    }
  } else {
    tracing::warn!("[ipc] open_url blocked disallowed scheme: {url}");
  }
}

#[tauri::command]
pub fn copy_to_clipboard(text: String, app: AppHandle) {
  if let Err(e) = app.clipboard().write_text(&text) {
    tracing::warn!("[ipc] copy_to_clipboard failed: {e}");
  }
}
