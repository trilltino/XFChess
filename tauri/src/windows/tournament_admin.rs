// Simple tournament admin window management
use tauri::{AppHandle, Manager, Result};

pub struct TournamentAdminWindow;

impl TournamentAdminWindow {
  pub fn new(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("tournament-admin") {
      window.show()?;
    }
    Ok(())
  }
}
