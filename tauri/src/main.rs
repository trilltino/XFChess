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
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tokio::io::AsyncReadExt;
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

// ---------------------------------------------------------------------------
// Shared State
// ---------------------------------------------------------------------------

/// Wallet public key in base58 format.
#[allow(dead_code)]
#[derive(Default, Clone)]
struct WalletPubkey(Arc<Mutex<Option<String>>>);

/// Username associated with the connected wallet.
#[derive(Default, Clone)]
struct WalletUsername(Arc<Mutex<Option<String>>>);

/// Type alias for in-flight signing request.
type PendingTxInner = Option<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, String>>)>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

/// Get the HTTP port for the wallet signing service.
fn http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

/// Get the backend API base URL.
fn get_backend_url() -> String {
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string())
}

/// Path to the consent record on disk.
fn consent_path() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("xfchess")
    .join("consent.json")
}

/// Path where the last connected wallet pubkey is persisted.
fn wallet_cache_path() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("xfchess")
    .join("wallet.json")
}

/// Load previously persisted wallet from disk. Returns (pubkey, username).
fn load_persisted_wallet() -> Option<(String, Option<String>)> {
  let bytes = std::fs::read(wallet_cache_path()).ok()?;
  let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
  let pubkey = json["pubkey"].as_str()?.to_string();
  let username = json["username"].as_str().map(|s| s.to_string());
  Some((pubkey, username))
}

/// Persist the wallet pubkey (and optional username) to disk so it survives Tauri restarts.
fn save_persisted_wallet(pubkey: &str, username: Option<&str>) {
  let path = wallet_cache_path();
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  let mut obj = serde_json::json!({ "pubkey": pubkey });
  if let Some(u) = username {
    obj["username"] = serde_json::Value::String(u.to_string());
  }
  let _ = std::fs::write(&path, obj.to_string());
}

/// Redirect authentication pages to the onboarding single-page application.
#[allow(dead_code)]
async fn redirect_to_onboard() -> impl IntoResponse {
  axum::response::Redirect::to("/onboard")
}

// ---------------------------------------------------------------------------
// In-process HTTP bridge — serves /pending, /resolved, /wallet, /hide,
// and proxies /api/** calls to the Hetzner backend at :8090.
// The wallet-ui React app polls and posts against http://localhost:7454.
// ---------------------------------------------------------------------------

async fn http_server(app: tauri::AppHandle, pending: PendingTx, wallet_pubkey: WalletPubkey, wallet_username: WalletUsername) {
  use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
  };
  use tower_http::cors::{Any, CorsLayer};

  #[derive(Clone)]
  struct LocalState {
    app: tauri::AppHandle,
    pending: PendingTx,
    wallet_pubkey: WalletPubkey,
    wallet_username: WalletUsername,
  }

  // GET /pending — wallet-ui polls; returns {"tx":"<b64>"} or {"tx":null}
  async fn get_pending(State(s): State<LocalState>) -> impl IntoResponse {
    let lock = s.pending.lock().unwrap();
    let tx_b64 = lock.as_ref().map(|(bytes, _)| B64.encode(bytes));
    if tx_b64.is_some() {
      // ensure popup is visible when a signing request arrives
      if let Some(win) = s.app.get_webview_window("wallet-popup") {
        let _ = win.show();
        let _ = win.set_focus();
      }
    }
    Json(serde_json::json!({ "tx": tx_b64 }))
  }

  // POST /resolved — wallet-ui posts {"signed":"<b64>"} after signing
  async fn post_resolved(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let signed_b64 = body["signed"].as_str().unwrap_or("").to_string();
    let mut lock = s.pending.lock().unwrap();
    if let Some((_, sender)) = lock.take() {
      if signed_b64.is_empty() {
        let _ = sender.send(Err("User cancelled".to_string()));
      } else {
        match B64.decode(&signed_b64) {
          Ok(bytes) => { let _ = sender.send(Ok(bytes)); }
          Err(e) => { let _ = sender.send(Err(format!("base64 decode: {e}"))); }
        }
      }
    }
    StatusCode::OK
  }

  // POST /wallet — wallet-ui posts {"pubkey":"<base58>","username":"<name>"} on wallet connect
  async fn post_wallet(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    if let Some(pk) = body["pubkey"].as_str() {
      let username = body["username"].as_str().unwrap_or("").to_string();
      *s.wallet_pubkey.0.lock().unwrap() = Some(pk.to_string());
      if !username.is_empty() {
        *s.wallet_username.0.lock().unwrap() = Some(username.clone());
      }
      tracing::info!("[HTTP] Wallet connected: {pk} username={username}");
      save_persisted_wallet(pk, if username.is_empty() { None } else { Some(&username) });
    }
    StatusCode::OK
  }

  // POST /hide — no-op when wallet runs in Chrome (Chrome closes itself via window.close())
  async fn post_hide(_state: State<LocalState>) -> impl IntoResponse {
    StatusCode::OK
  }

  // GET /status — health / wallet info
  async fn get_status(State(s): State<LocalState>) -> impl IntoResponse {
    let pubkey = s.wallet_pubkey.0.lock().unwrap().clone();
    let username = s.wallet_username.0.lock().unwrap().clone();
    Json(serde_json::json!({ "connected": pubkey.is_some(), "pubkey": pubkey, "username": username }))
  }

  // GET /api/consent
  async fn api_get_consent() -> impl IntoResponse {
    let path = consent_path();
    match std::fs::read_to_string(&path)
      .ok()
      .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
      Some(v) => Json(v).into_response(),
      None => Json(serde_json::Value::Null).into_response(),
    }
  }

  // POST /api/consent
  async fn api_post_consent(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let version = body["version"].as_u64().unwrap_or(1) as u8;
    let ts = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs();
    let record = serde_json::json!({ "version": version, "accepted_at": ts });
    let path = consent_path();
    if let Some(parent) = path.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, record.to_string());
    StatusCode::OK
  }

  // Generic proxy helpers
  async fn proxy_post(url: &str, body: serde_json::Value) -> axum::response::Response {
    let client = reqwest::Client::new();
    match client.post(url).json(&body).send().await {
      Ok(resp) => {
        let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        match resp.json::<serde_json::Value>().await {
          Ok(v) => (status, Json(v)).into_response(),
          Err(e) => (status, e.to_string()).into_response(),
        }
      }
      Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
  }

  async fn proxy_post_auth(
    url: &str,
    body: serde_json::Value,
    headers: &axum::http::HeaderMap,
  ) -> axum::response::Response {
    let client = reqwest::Client::new();
    let mut req = client.post(url).json(&body);
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
      if let Ok(v) = auth.to_str() {
        req = req.header("Authorization", v);
      }
    }
    match req.send().await {
      Ok(resp) => {
        let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        match resp.json::<serde_json::Value>().await {
          Ok(v) => (status, Json(v)).into_response(),
          Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
      }
      Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
  }

  // Auth proxy routes
  async fn api_login(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/login", get_backend_url()), body).await
  }
  async fn api_register(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/register", get_backend_url()), body).await
  }
  async fn api_login_email(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/login-email", get_backend_url()), body).await
  }
  async fn api_register_email(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/register-email", get_backend_url()), body).await
  }
  async fn api_link_wallet(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/link-wallet", get_backend_url()), body).await
  }
  async fn api_sync_profile(headers: axum::http::HeaderMap) -> impl IntoResponse {
    proxy_post_auth(
      &format!("{}/api/auth/sync-profile", get_backend_url()),
      serde_json::Value::Null,
      &headers,
    )
    .await
  }
  async fn api_add_email(
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post_auth(&format!("{}/api/auth/add-email", get_backend_url()), body, &headers).await
  }
  async fn api_me(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/me", get_backend_url());
    let mut req = client.get(&url);
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
      if let Ok(v) = auth.to_str() {
        req = req.header("Authorization", v);
      }
    }
    match req.send().await {
      Ok(resp) => {
        let status =
          StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        match resp.json::<serde_json::Value>().await {
          Ok(v) => (status, Json(v)).into_response(),
          Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
      }
      Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
  }
  async fn api_set_username(
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/username", get_backend_url());
    let mut req = client.patch(&url).json(&body);
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
      if let Ok(v) = auth.to_str() {
        req = req.header("Authorization", v);
      }
    }
    match req.send().await {
      Ok(resp) => {
        let status =
          StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        match resp.json::<serde_json::Value>().await {
          Ok(v) => (status, Json(v)).into_response(),
          Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
      }
      Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
  }

  // Generic passthrough for remaining /api/** calls to backend
  async fn api_check_wallet(
    axum::extract::Path(pubkey): axum::extract::Path<String>,
  ) -> impl IntoResponse {
    let url = format!("{}/api/auth/check-wallet/{pubkey}", get_backend_url());
    match reqwest::get(&url).await {
      Ok(resp) => {
        let status =
          StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        match resp.json::<serde_json::Value>().await {
          Ok(v) => (status, Json(v)).into_response(),
          Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
      }
      Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
  }

  // Proxy /tournament-admin/* → vite dev server on :7455 (dev) or built dist (prod).
  // In dev the tournament-admin vite server runs on 7455 to avoid conflicting with
  // this bridge which owns :7454.
  async fn proxy_tournament_admin(
    uri: axum::http::Uri,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
  ) -> impl IntoResponse {
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/tournament-admin/");
    let target = format!("http://127.0.0.1:7455{path}");
    let client = reqwest::Client::new();
    let mut req = client.request(
      reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
      &target,
    );
    for (k, v) in &headers {
      if let Ok(v) = v.to_str() {
        req = req.header(k.as_str(), v);
      }
    }
    req = req.body(body);
    match req.send().await {
      Ok(resp) => {
        let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let headers_out = resp.headers().clone();
        match resp.bytes().await {
          Ok(bytes) => {
            let mut response = axum::response::Response::new(axum::body::Body::from(bytes));
            *response.status_mut() = status;
            for (k, v) in &headers_out {
              response.headers_mut().insert(k, v.clone());
            }
            response
          }
          Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
        }
      }
      Err(_) => {
        // Dev server not running — serve a helpful error
        (StatusCode::SERVICE_UNAVAILABLE,
          "Tournament admin vite dev server not running. Run: just admin").into_response()
      }
    }
  }

  let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([Method::GET, Method::POST, axum::http::Method::PATCH, axum::http::Method::DELETE, axum::http::Method::OPTIONS])
    .allow_headers(Any);

  let state = LocalState {
    app,
    pending,
    wallet_pubkey,
    wallet_username,
  };

  let router = Router::new()
    .route("/pending", get(get_pending))
    .route("/resolved", post(post_resolved))
    .route("/wallet", post(post_wallet))
    .route("/hide", post(post_hide))
    .route("/status", get(get_status))
    .route("/api/consent", get(api_get_consent).post(api_post_consent))
    .route("/api/auth/login", post(api_login))
    .route("/api/auth/register", post(api_register))
    .route("/api/auth/login-email", post(api_login_email))
    .route("/api/auth/register-email", post(api_register_email))
    .route("/api/auth/link-wallet", post(api_link_wallet))
    .route("/api/auth/sync-profile", post(api_sync_profile))
    .route("/api/auth/add-email", post(api_add_email))
    .route("/api/auth/me", get(api_me))
    .route("/api/auth/username", axum::routing::patch(api_set_username))
    .route("/api/auth/check-wallet/{pubkey}", get(api_check_wallet))
    // Proxy tournament admin UI (vite dev on :7455, or built dist in prod)
    .route("/tournament-admin", axum::routing::any(proxy_tournament_admin))
    .route("/tournament-admin/", axum::routing::any(proxy_tournament_admin))
    .route("/tournament-admin/{*path}", axum::routing::any(proxy_tournament_admin))
    .layer(cors)
    .with_state(state);

  let port = http_port();
  let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
  tracing::info!("[HTTP] Wallet bridge listening on http://localhost:{port}");
  match TcpListener::bind(addr).await {
    Ok(listener) => {
      if let Err(e) = axum::serve(listener, router).await {
        tracing::error!("[HTTP] Wallet bridge error: {e}");
      }
    }
    Err(e) => tracing::error!("[HTTP] Failed to bind wallet bridge on :{port}: {e}"),
  }
}

/// Open the wallet UI in the user's real Chrome browser so Phantom/Solflare
/// extensions are available. WebView2 inside Tauri cannot load extensions.
fn open_wallet_popup(_app: &tauri::AppHandle) {
  let wallet_url = std::env::var("XFCHESS_WALLET_URL")
    .unwrap_or_else(|_| "http://localhost:5174".to_string());
  tracing::info!("[WalletPopup] opening in system browser: {wallet_url}");
  open_in_browser(&wallet_url);
}

/// Open a URL in Chrome app-mode (compact popup, no address bar).
/// Falls back to the system default browser if Chrome is not found.
fn open_in_browser(url: &str) {
  let ts = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let url_ts = format!("{url}?_t={ts}");

  #[cfg(windows)]
  {
    let chrome_paths = [
      r"C:\Program Files\Google\Chrome\Application\chrome.exe",
      r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    let app_flag = format!("--app={}", url_ts);
    for path in &chrome_paths {
      if std::path::Path::new(path).exists() {
        let _ = Command::new(path)
          .args([&app_flag, "--window-size=420,600"])
          .spawn();
        return;
      }
    }
    // Fallback: default browser
    let _ = Command::new("cmd").args(["/C", "start", "", &url_ts]).spawn();
  }
  #[cfg(not(windows))]
  {
    let _ = Command::new("xdg-open").arg(&url_ts).spawn();
  }
}

#[tauri::command]
fn show_wallet_popup_window(app: tauri::AppHandle) {
  tracing::info!("[WalletPopup] show_wallet_popup_window invoked");
  open_wallet_popup(&app);
}

fn open_tournament_admin(app: &tauri::AppHandle) {
  if let Some(win) = app.get_webview_window("tournament-admin") {
    tracing::info!("[TournamentAdmin] showing existing window");
    let _ = win.show();
    let _ = win.set_focus();
  } else {
    tracing::info!("[TournamentAdmin] creating new window");
    let url = tauri::WebviewUrl::External(
      "http://localhost:7454/tournament-admin/".parse().expect("valid URL"),
    );
    match tauri::WebviewWindowBuilder::new(app, "tournament-admin", url)
      .title("XFChess Tournament Admin")
      .inner_size(1200.0, 800.0)
      .min_inner_size(800.0, 600.0)
      .resizable(true)
      .decorations(true)
      .center()
      .build()
    {
      Ok(win) => { let _ = win.set_focus(); }
      Err(e) => tracing::error!("[TournamentAdmin] failed to create window: {e}"),
    }
  }
}

#[tauri::command]
fn show_tournament_admin_window(app: tauri::AppHandle) {
  tracing::info!("[TournamentAdmin] show_tournament_admin_window invoked");
  open_tournament_admin(&app);
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
      // Always start disconnected — user must connect a wallet each session.
      let wallet_pubkey = WalletPubkey::default();
      let wallet_username = WalletUsername::default();
      let pending_tx: PendingTx = Arc::new(Mutex::new(None));
      let auth_state = services::auth::AuthState::new();

      // Register shared state with Tauri app
      app.manage(wallet_pubkey.clone());
      app.manage(wallet_username.clone());
      app.manage(pending_tx.clone());
      app.manage(auth_state);

      // ── HTTP wallet bridge — /pending, /resolved, /wallet, /hide ──────────
      // The wallet-ui React app polls http://localhost:7454/pending for unsigned
      // transactions and posts signed results back. This in-process server is
      // the glue between the Tauri wallet popup and the game client.
      {
        let h = app.handle().clone();
        let p = pending_tx.clone();
        let w = wallet_pubkey.clone();
        let wu = wallet_username.clone();
        tauri::async_runtime::spawn(http_server(h, p, w, wu));
      }

      // Initialize windows
      let handle = app.handle();

      #[cfg(feature = "tournament-admin")]
      {
        let _ = TournamentAdminWindow::new(handle);
      }

      // ── Wallet Bridge TCP listener ──────────────────────────────────────────
      // Listens for "OPEN" sent by the game client to trigger the wallet popup.
      {
        let app_handle = app.handle().clone();
        let base_port: u16 = std::env::var("XFCHESS_WALLET_PORT")
          .ok().and_then(|v| v.parse().ok()).unwrap_or(7454);
        std::thread::spawn(move || {
          let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().expect("[WalletBridge] tokio runtime");
          rt.block_on(async move {
            // Try binding on ports base-11 through base-2
            let mut listener = None;
            for offset in 2u16..=11 {
              let port = base_port.saturating_sub(offset);
              if let Ok(l) = TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                tracing::info!("[WalletBridge] Listening on port {}", port);
                listener = Some(l);
                break;
              }
            }
            let listener = match listener {
              Some(l) => l,
              None => { tracing::warn!("[WalletBridge] No port available"); return; }
            };
            loop {
              if let Ok((mut stream, _)) = listener.accept().await {
                let app2 = app_handle.clone();
                tokio::spawn(async move {
                  let mut buf = [0u8; 16];
                  if let Ok(n) = stream.read(&mut buf).await {
                    if &buf[..n.min(4)] == b"OPEN" {
                      open_wallet_popup(&app2);
                    }
                  }
                });
              }
            }
          });
        });
      }

      // ── Background Notification Poller ──────────────────────────────────────
      let backend_url = std::env::var("VITE_BACKEND_URL")
        .unwrap_or_else(|_| "http://localhost:8090".to_string());
      let pubkey_for_poller = wallet_pubkey.0.lock().unwrap().clone();
      services::notification_poller::startPoller(
        app.handle().clone(),
        backend_url,
        pubkey_for_poller,
      );

      // ── System Tray ────────────────────────────────────────────────────────
      let tray_menu = tauri::menu::MenuBuilder::new(app)
        .item(&tauri::menu::MenuItemBuilder::new("Show XFChess").id("show").build(app)?)
        .separator()
        .item(&tauri::menu::MenuItemBuilder::new("Tournaments").id("tournaments").build(app)?)
        .item(&tauri::menu::MenuItemBuilder::new("Matchmaking").id("matchmaking").build(app)?)
        .item(&tauri::menu::MenuItemBuilder::new("Tournament Admin").id("tournament-admin").build(app)?)
        .separator()
        .item(&tauri::menu::MenuItemBuilder::new("Quit").id("quit").build(app)?)
        .build()?;

      let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("no default window icon configured"))
        .tooltip("XFChess")
        .menu(&tray_menu)
        .menu_on_left_click(true)
        .on_menu_event(|app, event| {
          match event.id().as_ref() {
            "show" => {
              if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
              }
            }
            "tournaments" => {
              if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.eval("window.location.href = '/tournaments';");
              }
            }
            "matchmaking" => {
              if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.eval("window.location.href = '/pvp';");
              }
            }
            "tournament-admin" => {
              open_tournament_admin(app);
            }
            "quit" => {
              app.exit(0);
            }
            _ => {}
          }
        })
        .on_tray_icon_event(|tray, event| {
          if let TrayIconEvent::DoubleClick { .. } = event {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
              let _ = window.show();
              let _ = window.set_focus();
            }
          }
        })
        .build(app)?;

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
