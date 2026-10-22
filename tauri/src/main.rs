#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use parking_lot::Mutex;
use std::sync::Arc;
use tauri::Manager;

mod wallet;
use wallet::*;

mod services;
mod utils;
mod windows;

use utils::logging::init_logging;
use windows::tournament_admin::open_tournament_admin;
#[cfg(feature = "tournament-admin")]
use windows::tournament_admin::{kill_admin_tunnel_inner, AdminTunnel};

#[tauri::command]
fn show_wallet_popup_window(app: tauri::AppHandle) {
  tracing::info!("[WalletPopup] show_wallet_popup_window invoked");
  open_wallet_popup(&app);
}

// Consumer builds omit tournament-admin and cannot create this window.
fn apply_xfchess_window_icon(window: &tauri::WebviewWindow, app: &tauri::AppHandle) {
  let icon = app
    .default_window_icon()
    .cloned()
    .or_else(|| tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")).ok());
  if let Some(icon) = icon {
    if let Err(e) = window.set_icon(icon) {
      tracing::warn!(
        "[WindowIcon] failed to set icon for {}: {e}",
        window.label()
      );
    }
  } else {
    tracing::warn!("[WindowIcon] no XFChess window icon was available");
  }
}

#[tauri::command]
fn show_tournament_admin_window(app: tauri::AppHandle) {
  tracing::info!("[TournamentAdmin] show_tournament_admin_window invoked");
  open_tournament_admin(&app);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  init_logging();

  tauri::Builder::default()
    .plugin(tauri_plugin_deep_link::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_shell::init())
    // Allow native admin HTTP calls only to loopback ports through admin-http capabilities.
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    .setup(|app| {
      // Always start disconnected — user must connect a wallet each session.
      let wallet_pubkey = WalletPubkey::default();
      let wallet_username = WalletUsername::default();
      let wallet_provider = WalletProvider::default();
      let wallet_jwt = WalletJwt::default();
      let pending_tx: PendingTx = Arc::new(Mutex::new(None));
      let (pending_notify, _): (PendingTxNotify, _) = tokio::sync::watch::channel(());

      app.manage(wallet_pubkey.clone());
      app.manage(wallet_username);
      app.manage(wallet_provider);
      app.manage(wallet_jwt);
      app.manage(pending_tx.clone());
      app.manage(pending_notify.clone());
      #[cfg(feature = "tournament-admin")]
      app.manage(AdminTunnel::default());

      if let Some(win) = app.get_webview_window("main") {
        apply_xfchess_window_icon(&win, app.handle());
      }

      // The wallet UI receives pending transactions over SSE and posts resolutions;
      // /token exposes its authenticated JWT to the game.
      {
        let (listener, _) = tauri::async_runtime::block_on(bind_http_port())
          .ok_or("No wallet HTTP port is available")?;
        tauri::async_runtime::spawn(http_server(listener, app.handle().clone()));
      }

      spawn_wallet_popup_idle_reaper();

      // Retry admin window creation until the event loop is ready.
      // Only development builds check the auto-open environment variable.
      #[cfg(feature = "tournament-admin")]
      if std::env::var("XFCHESS_OPEN_ADMIN").is_ok_and(|v| v == "1") {
        let h = app.handle().clone();
        tauri::async_runtime::spawn(async move {
          for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if h.get_webview_window("tournament-admin").is_some() {
              break;
            }
            open_tournament_admin(&h);
          }
        });
      }

      start_tcp_bridge(app.handle(), &pending_tx, &pending_notify, &wallet_pubkey);

      if let Ok(pid) = std::env::var("XFCHESS_GAME_PID") {
        wallet::attach_game(
          pid.parse()?,
          app.handle().clone(),
          pending_tx,
          pending_notify,
        )
        .map_err(std::io::Error::other)?;
      }

      // Keep backend defaults aligned with the game client and wallet bridge.
      let backend_url = std::env::var("VITE_BACKEND_URL")
        .or_else(|_| std::env::var("SIGNING_SERVICE_URL"))
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| "https://xfchess.com".to_string());
      services::notification_poller::start_poller(
        app.handle().clone(),
        backend_url,
        wallet_pubkey.0,
      );

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
      services::ipc::toggle_maximize_tournament_admin,
      services::ipc::is_tournament_admin_maximized,
      services::ipc::close_tournament_admin,
      services::ipc::show_notification,
      services::ipc::open_url,
      services::ipc::copy_to_clipboard,
      #[cfg(feature = "tournament-admin")]
      windows::tournament_admin::ensure_admin_tunnel,
      #[cfg(feature = "tournament-admin")]
      windows::tournament_admin::kill_admin_tunnel,
      #[cfg(feature = "tournament-admin")]
      windows::tournament_admin::admin_token_load,
      #[cfg(feature = "tournament-admin")]
      windows::tournament_admin::admin_token_save,
      #[cfg(feature = "tournament-admin")]
      windows::tournament_admin::admin_token_clear,
    ])
    .build(tauri::generate_context!())?
    .run(|_app_handle, event| {
      if matches!(
        event,
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
      ) {
        wallet::shutdown(
          &_app_handle.state::<PendingTx>(),
          &_app_handle.state::<PendingTxNotify>(),
        );
        // Release the admin tunnel's listening port on exit.
        #[cfg(feature = "tournament-admin")]
        kill_admin_tunnel_inner(_app_handle);
      }
    });
  Ok(())
}
