#![allow(dead_code)]
use tauri::{AppHandle, Manager, Result, WebviewUrl, WebviewWindowBuilder};

use crate::services::ipc::IpcServer;

pub struct WalletWindow;

impl WalletWindow {
  pub fn new(app: &AppHandle) -> Result<()> {
    let window = WebviewWindowBuilder::new(
      app,
      "wallet-popup",
      WebviewUrl::App("wallet/index.html".into()),
    )
    .title("XFChess Wallet")
    .inner_size(420.0, 500.0)
    .resizable(false)
    .decorations(false)
    .transparent(false)
    .always_on_top(true)
    .center()
    .skip_taskbar(false)
    .build()?;

    let ipc_server = IpcServer::new();
    window.manage(ipc_server);

    Ok(())
  }

  pub fn show(window: &tauri::Window) -> Result<()> {
    window.show()?;
    window.set_focus()?;
    window.center()?;
    Ok(())
  }

  pub fn hide(window: &tauri::Window) -> Result<()> {
    window.hide()?;
    Ok(())
  }

  pub fn set_always_on_top(window: &tauri::Window, always_on_top: bool) -> Result<()> {
    window.set_always_on_top(always_on_top)?;
    Ok(())
  }
}
