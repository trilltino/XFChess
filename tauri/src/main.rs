// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tauri::Manager;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Wallet pubkey in base58 — written by the React app after Phantom connects.
#[derive(Default, Clone)]
struct WalletPubkey(Arc<Mutex<Option<String>>>);

/// In-flight signing request: raw tx bytes + a channel to return signed bytes.
type PendingTxInner = Option<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, String>>)>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

/// HTTP port for the wallet signing page.
/// Reads XFCHESS_WALLET_PORT env var; defaults to 7454.
fn http_port() -> u16 {
    std::env::var("XFCHESS_WALLET_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7454)
}

/// Global flag: has the browser tab already been opened?
static BROWSER_OPENED: AtomicBool = AtomicBool::new(false);

/// Wallet popup dimensions.
const WALLET_WIDTH: u32 = 360;
const WALLET_HEIGHT: u32 = 520;

/// Open the wallet page in a compact browser popup — but only once.
/// The React page polls `/pending` for new signing requests, so
/// subsequent transactions are handled without opening extra tabs.
///
/// Tries Chrome/Edge `--app` mode first (clean popup, no address bar).
/// Falls back to the system default browser if neither is found.
fn open_browser_once() {
    if BROWSER_OPENED.swap(true, Ordering::SeqCst) {
        return;
    }
    let url = format!("http://localhost:{}", http_port());
    let size_arg = format!("--window-size={},{}", WALLET_WIDTH, WALLET_HEIGHT);

    // Common Chrome / Edge paths on Windows
    let candidates: &[&str] = &[
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            let app_url = format!("--app={}", url);
            if Command::new(path)
                .args([&app_url, &size_arg, "--window-position=100,100"])
                .spawn()
                .is_ok()
            {
                println!("[WALLET] Opened popup via {}", path);
                return;
            }
        }
    }

    // Fallback: system default browser (full tab)
    println!("[WALLET] No Chrome/Edge found, falling back to default browser");
    let _ = open::that(url);
}

// ---------------------------------------------------------------------------
// HTTP signing bridge (axum)
// ---------------------------------------------------------------------------

async fn http_server(pending: PendingTx, wallet_pubkey: WalletPubkey) {
    use axum::{
        body::Body,
        extract::State,
        http::{header, Method, StatusCode},
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    };
    use tower_http::cors::{Any, CorsLayer};

    #[derive(Clone)]
    struct AppState {
        pending: PendingTx,
        wallet_pubkey: WalletPubkey,
    }

    // GET /pending — React polls this; returns {"tx": "<base64>"} or {"tx": null}
    async fn get_pending(
        State(state): State<AppState>,
    ) -> impl IntoResponse {
        let lock = state.pending.lock().unwrap();
        let tx_b64 = lock.as_ref().map(|(bytes, _)| B64.encode(bytes));
        Json(serde_json::json!({ "tx": tx_b64 }))
    }

    // POST /resolved — React posts {"signed": "<base64>"} after Phantom signs
    async fn post_resolved(
        State(state): State<AppState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        let signed_b64 = body["signed"].as_str().unwrap_or("").to_string();
        let mut lock = state.pending.lock().unwrap();
        if let Some((_, sender)) = lock.take() {
            if signed_b64.is_empty() {
                let _ = sender.send(Err("User cancelled".to_string()));
            } else {
                match B64.decode(&signed_b64) {
                    Ok(bytes) => { let _ = sender.send(Ok(bytes)); }
                    Err(e) => { let _ = sender.send(Err(format!("base64 decode: {}", e))); }
                }
            }
        }
        StatusCode::OK
    }

    // POST /wallet — React posts {"pubkey": "<base58>"} on wallet connect
    async fn post_wallet(
        State(state): State<AppState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        if let Some(pk) = body["pubkey"].as_str() {
            let mut lock = state.wallet_pubkey.0.lock().unwrap();
            *lock = Some(pk.to_string());
            println!("[HTTP] Wallet connected: {}", pk);
        }
        StatusCode::OK
    }

    // GET /status — health-check / wallet info for the React page
    async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
        let pubkey = state.wallet_pubkey.0.lock().unwrap().clone();
        Json(serde_json::json!({ "connected": pubkey.is_some(), "pubkey": pubkey }))
    }

    // Serve the built React SPA (index.html + assets) from wallet-ui/dist
    async fn serve_index() -> impl IntoResponse {
        let dist = dist_path();
        let html_path = dist.join("index.html");
        match tokio::fs::read_to_string(&html_path).await {
            Ok(content) => Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(content))
                .unwrap(),
            Err(_) => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Wallet UI not built. Run: cd tauri/wallet-ui && npm run build"))
                .unwrap(),
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let state = AppState {
        pending: pending.clone(),
        wallet_pubkey: wallet_pubkey.clone(),
    };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/pending", get(get_pending))
        .route("/resolved", post(post_resolved))
        .route("/wallet", post(post_wallet))
        .route("/status", get(get_status))
        .nest_service(
            "/assets",
            tower_http::services::ServeDir::new(dist_path().join("assets")),
        )
        .layer(cors)
        .with_state(state);

    let port = http_port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    println!("[HTTP] Wallet signing server on http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ---------------------------------------------------------------------------
// TCP signing bridge (Bevy ↔ Tauri)
// ---------------------------------------------------------------------------

/// TCP signing server — listens on 127.0.0.1:7443-7452.
///
/// Protocol:
///   Client → Server : b"PKEY"                        → pubkey query
///   Server → Client : 4-byte LE len + pubkey bytes
///
///   Client → Server : 4-byte LE len + raw tx bytes   → signing request
///   Server → Client : 4-byte LE len + signed bytes   (success)
///                  or 0xFFFF_FFFF                    (error / timeout)
async fn tcp_signing_server(pending: PendingTx, wallet_pubkey: WalletPubkey) {
    let listener = {
        let base = http_port();
        let tcp_start = base.saturating_sub(11);
        let tcp_end = base.saturating_sub(2);
        let mut found = None;
        for port in tcp_start..=tcp_end {
            if let Ok(l) = TcpListener::bind(("127.0.0.1", port)).await {
                println!("[SIGN-SRV] TCP bridge on 127.0.0.1:{}", port);
                found = Some(l);
                break;
            }
        }
        match found {
            Some(l) => l,
            None => {
                eprintln!("[SIGN-SRV] No port available in {}-{}", tcp_start, tcp_end);
                return;
            }
        }
    };

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(p) => p,
            Err(e) => { eprintln!("[SIGN-SRV] accept: {}", e); continue; }
        };

        let pending = pending.clone();
        let wallet_pubkey = wallet_pubkey.clone();

        tauri::async_runtime::spawn(async move {
            let (mut reader, mut writer) = stream.into_split();

            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).await.is_err() {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }

            // OPEN command — open the wallet page in Chrome (once).
            if &len_buf == b"OPEN" {
                open_browser_once();
                return;
            }

            // PKEY query — return the wallet pubkey as a UTF-8 base58 string.
            if &len_buf == b"PKEY" {
                let pubkey_str = wallet_pubkey.0.lock().unwrap().clone();
                match pubkey_str {
                    Some(s) => {
                        let bytes = s.into_bytes();
                        let _ = writer.write_all(&(bytes.len() as u32).to_le_bytes()).await;
                        let _ = writer.write_all(&bytes).await;
                    }
                    None => {
                        let _ = writer.write_all(&0u32.to_le_bytes()).await;
                    }
                }
                return;
            }

            // Transaction signing request.
            let tx_len = u32::from_le_bytes(len_buf) as usize;
            if tx_len == 0 || tx_len > 4 * 1024 * 1024 {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }
            let mut tx_bytes = vec![0u8; tx_len];
            if reader.read_exact(&mut tx_bytes).await.is_err() {
                let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                return;
            }

            // Store the pending tx and show the wallet popup.
            let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();
            {
                let mut lock = pending.lock().unwrap();
                *lock = Some((tx_bytes, tx));
            }

            println!("[SIGN-SRV] Pending signing request — wallet page will pick it up");
            open_browser_once();

            // Wait for the React page to POST the signed bytes (60 s timeout).
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                rx,
            )
            .await;

            match result {
                Ok(Ok(Ok(signed))) => {
                    let _ = writer.write_all(&(signed.len() as u32).to_le_bytes()).await;
                    let _ = writer.write_all(&signed).await;
                }
                Ok(Ok(Err(e))) => {
                    eprintln!("[SIGN-SRV] Signing rejected: {}", e);
                    let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                }
                _ => {
                    eprintln!("[SIGN-SRV] Signing timed out or channel dropped");
                    let _ = writer.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bs58_to_bytes(s: &str) -> Option<Vec<u8>> {
    let alphabet = bs58::Alphabet::DEFAULT;
    bs58::decode(s).with_alphabet(alphabet).into_vec().ok()
}

/// Path to the built React SPA.
fn dist_path() -> PathBuf {
    // Relative to the Tauri binary: look for wallet-ui/dist next to it, or
    // fall back to the source tree location used during development.
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(ref dir) = self_dir {
        let candidate = dir.join("wallet-ui").join("dist");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("tauri/wallet-ui/dist")
}

fn spawn_bevy_game(wallet_mode: bool) -> Result<std::process::Child, String> {
    let exe_name = if cfg!(windows) { "xfchess.exe" } else { "xfchess" };

    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let candidate_paths = [
        self_dir.as_ref().map(|d| d.join(exe_name)),
        Some(PathBuf::from(format!("target/debug/{}", exe_name))),
        Some(PathBuf::from(format!("target/release/{}", exe_name))),
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
    }
    cmd.spawn().map_err(|e| format!("Failed to launch game: {}", e))
}

// ---------------------------------------------------------------------------
// Tauri commands (kept for API compatibility with any existing IPC calls)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_wallet_status() -> serde_json::Value {
    serde_json::json!({ "connected": false, "pubkey": null })
}

#[tauri::command]
async fn wallet_connect() -> Result<String, String> {
    Ok("open_browser".to_string())
}

#[tauri::command]
async fn resolve_signed_tx(_signed_b64: String) -> Result<(), String> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let pending: PendingTx = Arc::new(Mutex::new(None));
    let wallet_pubkey = WalletPubkey::default();

    let pending_for_setup = pending.clone();
    let pubkey_for_setup = wallet_pubkey.clone();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_wallet_status,
            wallet_connect,
            resolve_signed_tx,
        ])
        .setup(move |app| {
            // Start HTTP wallet server.
            let p = pending_for_setup.clone();
            let w = pubkey_for_setup.clone();
            tauri::async_runtime::spawn(http_server(p, w));

            // Start TCP signing bridge.
            let p2 = pending_for_setup.clone();
            let w2 = pubkey_for_setup.clone();
            tauri::async_runtime::spawn(tcp_signing_server(p2, w2));

            // Launch the Bevy game and exit when it closes.
            let wallet_mode = std::env::var("XFCHESS_SOLANA").unwrap_or_default() == "1";
            match spawn_bevy_game(wallet_mode) {
                Ok(mut child) => {
                    let pid = child.id();
                    println!("[TAURI] Bevy game launched (PID {})", pid);
                    std::thread::spawn(move || {
                        let _ = child.wait();
                        println!("[TAURI] Game exited — shutting down");
                        std::process::exit(0);
                    });
                }
                Err(e) => eprintln!("[TAURI] Failed to launch game: {}", e),
            }

            // Hide the Tauri webview — wallet lives in the browser where
            // Chrome extensions (Phantom / Solflare) are available.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
