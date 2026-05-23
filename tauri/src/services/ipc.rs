use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

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
    window.show().unwrap();
    window.set_focus().unwrap();
  }
}

#[tauri::command]
pub fn hide_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.hide().unwrap();
  }
}

#[tauri::command]
pub fn set_tournament_admin_title(title: String, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.set_title(&title).unwrap();
  }
}

#[tauri::command]
pub fn set_tournament_admin_size(width: f64, height: f64, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    let size = tauri::Size::Physical(tauri::PhysicalSize {
      width: width as u32,
      height: height as u32,
    });
    window.set_size(size).unwrap();
  }
}

#[tauri::command]
pub fn set_tournament_admin_position(x: f64, y: f64, app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    let pos = tauri::Position::Physical(tauri::PhysicalPosition {
      x: x as i32,
      y: y as i32,
    });
    window.set_position(pos).unwrap();
  }
}

#[tauri::command]
pub fn minimize_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.minimize().unwrap();
  }
}

#[tauri::command]
pub fn maximize_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.maximize().unwrap();
  }
}

#[tauri::command]
pub fn close_tournament_admin(app: AppHandle) {
  if let Some(window) = app.get_webview_window("tournament-admin") {
    window.close().unwrap();
  }
}

#[tauri::command]
pub fn show_notification(title: String, body: String, app: AppHandle) {
  app
    .notification()
    .builder()
    .title(title)
    .body(body)
    .show()
    .unwrap();
}

#[tauri::command]
pub fn open_url(url: String, app: AppHandle) {
  app.shell().open(&url, None).unwrap();
}

#[tauri::command]
pub fn copy_to_clipboard(text: String, app: AppHandle) {
  app.clipboard().write_text(&text).unwrap();
}
