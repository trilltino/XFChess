#![allow(dead_code)]
use tauri::{AppHandle, Result, WebviewWindowBuilder, WebviewUrl};

pub struct PopupWindow;

impl PopupWindow {
  pub fn new(app: &AppHandle) -> Result<()> {
    let _window = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("popup/index.html".into()))
      .title("XFChess Popup")
      .inner_size(400.0, 300.0)
      .resizable(false)
      .decorations(true)
      .center()
      .always_on_top(true)
      .build()?;

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

  pub fn set_always_on_top(
    window: &tauri::Window,
    always_on_top: bool,
  ) -> Result<()> {
    window.set_always_on_top(always_on_top)?;
    Ok(())
  }
}
