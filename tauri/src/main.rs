// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use tauri::Manager;

/// Launch the native Bevy game as a sidecar process.
/// The game binary is built separately and bundled alongside Tauri.
fn spawn_bevy_game(wallet_mode: bool) -> Result<std::process::Child, String> {
    let exe_name = if cfg!(windows) {
        "xfchess.exe"
    } else {
        "xfchess"
    };

    // Look for the game binary next to this executable first,
    // then fall back to cargo-built target directory.
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let candidate_paths = [
        self_dir.as_ref().map(|d| d.join(exe_name)),
        Some(std::path::PathBuf::from(format!("target/debug/{}", exe_name))),
        Some(std::path::PathBuf::from(format!("target/release/{}", exe_name))),
    ];

    let game_path = candidate_paths
        .iter()
        .flatten()
        .find(|p| p.exists())
        .ok_or_else(|| format!("Could not find {} binary", exe_name))?
        .clone();

    let mut cmd = Command::new(&game_path);

    if wallet_mode {
        cmd.env("XFCHESS_WALLET_MODE", "tauri");
        cmd.args(["--features", "solana"]);
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))
}

/// Tauri command: request wallet connection from the webview.
#[tauri::command]
async fn wallet_connect() -> Result<String, String> {
    Ok("wallet_connect_requested".to_string())
}

/// Tauri command: request transaction signing from the webview.
#[tauri::command]
async fn sign_transaction(tx_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    // The webview JS will handle the actual signing via wallet adapter.
    // This command is a bridge — Bevy calls this, Tauri forwards to webview.
    Ok(tx_bytes)
}

/// Tauri command: get connection status.
#[tauri::command]
async fn get_wallet_status(state: tauri::State<'_, WalletState>) -> Result<WalletInfo, String> {
    let info = state.0.lock().map_err(|e| e.to_string())?;
    Ok(info.clone())
}

/// Tauri command: store wallet pubkey from webview after user connects.
#[tauri::command]
async fn set_wallet_pubkey(
    pubkey: String,
    state: tauri::State<'_, WalletState>,
) -> Result<(), String> {
    let mut info = state.0.lock().map_err(|e| e.to_string())?;
    info.pubkey = Some(pubkey);
    info.connected = true;
    Ok(())
}

/// Tauri command: disconnect wallet.
#[tauri::command]
async fn wallet_disconnect(state: tauri::State<'_, WalletState>) -> Result<(), String> {
    let mut info = state.0.lock().map_err(|e| e.to_string())?;
    info.pubkey = None;
    info.connected = false;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WalletInfo {
    connected: bool,
    pubkey: Option<String>,
}

impl Default for WalletInfo {
    fn default() -> Self {
        Self {
            connected: false,
            pubkey: None,
        }
    }
}

struct WalletState(std::sync::Mutex<WalletInfo>);

fn main() {
    tauri::Builder::default()
        .manage(WalletState(std::sync::Mutex::new(WalletInfo::default())))
        .invoke_handler(tauri::generate_handler![
            wallet_connect,
            sign_transaction,
            get_wallet_status,
            set_wallet_pubkey,
            wallet_disconnect,
        ])
        .setup(|app| {
            // Spawn the Bevy game process
            let wallet_mode = std::env::var("XFCHESS_SOLANA").unwrap_or_default() == "1";

            match spawn_bevy_game(wallet_mode) {
                Ok(child) => {
                    println!("[TAURI] Bevy game launched (PID: {})", child.id());
                }
                Err(e) => {
                    eprintln!("[TAURI] Failed to launch game: {}", e);
                }
            }

            // If wallet mode, the webview window is already created by Tauri config.
            // Otherwise, we could hide or minimize the wallet window.
            if !wallet_mode {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
