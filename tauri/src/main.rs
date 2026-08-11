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
use utils::logging::init_logging;
#[cfg(feature = "tournament-admin")]
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

/// JWT token issued by the backend on successful auth.
/// Shared between the bridge HTTP server and the main app handle.
#[derive(Default, Clone)]
struct WalletJwt(Arc<Mutex<Option<String>>>);

/// Type alias for in-flight signing request: the raw tx bytes, a short
/// human-readable label describing what's being signed (e.g. "Joining game"),
/// and the channel to deliver the signed result back to the blocked Bevy thread.
type PendingTxInner = Option<(Vec<u8>, String, oneshot::Sender<Result<Vec<u8>, String>>)>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

/// Change notification for `PendingTx` — fired (value is a no-op unit) every
/// time the pending slot transitions Some<->None, so `/pending/stream` (SSE)
/// can push instantly instead of wallet-ui polling `/pending` on a timer.
/// A `watch::Sender` doubles as the subscribe factory: each SSE connection
/// calls `.subscribe()` on a clone of this sender to get its own receiver.
type PendingTxNotify = tokio::sync::watch::Sender<()>;

/// How long to wait for the user to approve a transaction in the wallet
/// popup before giving up. Must match `SIGN_TIMEOUT_SECS` in the game
/// client's `src/multiplayer/solana/tauri_signer.rs` — that side sets the
/// same read timeout on its end of this same TCP connection.
const SIGN_TIMEOUT_SECS: u64 = 60;

/// Get the HTTP port for the wallet signing service.
fn http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

/// Path used to announce the raw-TCP wallet bridge's actual bound port to
/// the game client, keyed by `base_port` (XFCHESS_WALLET_PORT) so multiple
/// local instances (e.g. `just dev2`'s P1/P2) never collide on one file.
/// Must match `wallet_bridge_port_file()` in the game client's
/// `src/multiplayer/solana/tauri_signer.rs`.
fn wallet_bridge_port_file(base_port: u16) -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-bridge-{base_port}.port"))
}

/// Get the backend API base URL.
///
/// Defaults to the production Hetzner backend — same resolution order and
/// same default as the game client's `vps_base()` (`src/multiplayer/network/vps/client.rs`).
/// These two MUST stay in sync: if this ever defaults somewhere the game client
/// doesn't (e.g. a local dev server), every `/api/auth/*` call proxied through
/// this bridge silently 502s while the game client's own VPS calls succeed —
/// a confusing split-brain failure that looks like a server outage but isn't.
fn get_backend_url() -> String {
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "https://xfchess.com".to_string())
}

/// Per-instance cache directory, scoped by the wallet bridge port so that
/// two dev sidecars (e.g. XFCHESS_WALLET_PORT=7454 and 7464) never share
/// consent/wallet state.
fn instance_cache_dir() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("xfchess")
    .join(format!("port-{}", http_port()))
}

/// Path to the consent record on disk.
fn consent_path() -> PathBuf {
  instance_cache_dir().join("consent.json")
}

// ---------------------------------------------------------------------------
// In-process HTTP bridge — serves /pending, /resolved, /wallet, /hide,
// and proxies /api/** calls to the Hetzner backend at :8090.
// The wallet-ui React app polls and posts against http://localhost:7454.
// ---------------------------------------------------------------------------

async fn http_server(
  app: tauri::AppHandle,
  pending: PendingTx,
  notify: PendingTxNotify,
  wallet_pubkey: WalletPubkey,
  wallet_username: WalletUsername,
  wallet_jwt: WalletJwt,
) {
  use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
  };
  use futures::stream::{self, Stream};
  use std::convert::Infallible;
  use tower_http::cors::{AllowOrigin, Any, CorsLayer};

  #[derive(Clone)]
  struct LocalState {
    app: tauri::AppHandle,
    pending: PendingTx,
    notify: PendingTxNotify,
    wallet_pubkey: WalletPubkey,
    wallet_username: WalletUsername,
    wallet_jwt: WalletJwt,
    #[cfg(feature = "tournament-admin")]
    dist_path: std::path::PathBuf,
    wallet_ui_dist_path: std::path::PathBuf,
    needs_profile_step: Arc<std::sync::atomic::AtomicBool>,
  }

  fn pending_json(pending: &PendingTx) -> serde_json::Value {
    let lock = pending.lock().unwrap();
    let tx_b64 = lock.as_ref().map(|(bytes, _, _)| B64.encode(bytes));
    let label = lock.as_ref().map(|(_, label, _)| label.clone());
    serde_json::json!({ "tx": tx_b64, "label": label })
  }

  // GET /pending — wallet-ui polls; returns {"tx":"<b64>","label":"<str>"} or {"tx":null}
  // Kept alongside /pending/stream as a plain-fetch fallback (e.g. if SSE is
  // ever blocked by something in the user's environment).
  async fn get_pending(State(s): State<LocalState>) -> impl IntoResponse {
    let body = pending_json(&s.pending);
    if !body["tx"].is_null() {
      // ensure popup is visible when a signing request arrives
      if let Some(win) = s.app.get_webview_window("wallet-popup") {
        let _ = win.show();
        let _ = win.set_focus();
      }
    }
    Json(body)
  }

  // GET /pending/stream — SSE push. Emits the current pending state
  // immediately on (re)connect, then again every time `notify` fires (a new
  // tx arrives, or the slot is cleared by /resolved or a timeout). Replaces
  // wallet-ui's 1s poll loop so a new signing request is picked up the
  // instant it's queued instead of up to 1s later on average.
  async fn get_pending_stream(
    State(s): State<LocalState>,
  ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = s.notify.subscribe();
    let pending = s.pending.clone();
    let stream = stream::unfold((rx, pending, true), |(mut rx, pending, first)| async move {
      if !first && rx.changed().await.is_err() {
        return None;
      }
      let event = Event::default()
        .json_data(pending_json(&pending))
        .unwrap_or_else(|_| Event::default());
      Some((Ok(event), (rx, pending, false)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
  }

  // POST /resolved — wallet-ui posts {"signed":"<b64>"} after signing
  async fn post_resolved(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let signed_b64 = body["signed"].as_str().unwrap_or("").to_string();
    let mut lock = s.pending.lock().unwrap();
    if let Some((_, _, sender)) = lock.take() {
      if signed_b64.is_empty() {
        let _ = sender.send(Err("User cancelled".to_string()));
      } else {
        match B64.decode(&signed_b64) {
          Ok(bytes) => {
            let _ = sender.send(Ok(bytes));
          }
          Err(e) => {
            let _ = sender.send(Err(format!("base64 decode: {e}")));
          }
        }
      }
    }
    drop(lock);
    let _ = s.notify.send(());
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
    }
    StatusCode::OK
  }

  // POST /hide — hide (not kill) the wallet popup after a signature
  // resolves, so the next signing request can reuse the already-warm
  // process/page instead of paying a full respawn (window.close() from
  // inside is unreliable, see kill_wallet_popup doc-comment — same
  // EnumWindows-based approach is used to hide it). A background reaper
  // (see spawn_wallet_popup_idle_reaper) actually kills it after enough
  // idle time so a hidden popup never lingers forever.
  async fn post_hide(_state: State<LocalState>) -> impl IntoResponse {
    hide_wallet_popup();
    StatusCode::OK
  }

  // GET /status — health / wallet info
  async fn get_status(State(s): State<LocalState>) -> impl IntoResponse {
    let pubkey = s.wallet_pubkey.0.lock().unwrap().clone();
    let username = s.wallet_username.0.lock().unwrap().clone();
    Json(
      serde_json::json!({ "connected": pubkey.is_some(), "pubkey": pubkey, "username": username }),
    )
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
  // The backend is expected to always answer with JSON. If it doesn't (down,
  // mid-restart, returned an HTML/plain-text error page), surface that
  // plainly instead of forwarding reqwest's internal error text — that
  // showed up in the UI verbatim as "error decoding response body".
  fn backend_unreachable_msg(e: reqwest::Error) -> String {
    tracing::warn!("[HTTP] backend request failed: {e}");
    "Could not reach the backend service. Please check it's running and try again.".to_string()
  }
  fn backend_bad_response_msg(e: reqwest::Error) -> String {
    tracing::warn!("[HTTP] backend returned a non-JSON response: {e}");
    "The backend returned an unexpected response. Please try again in a moment.".to_string()
  }

  // Reads a backend response body exactly once and forwards it faithfully:
  // JSON stays JSON, anything else (plain-text bodies from the backend's
  // common `(StatusCode, String)` handler-error pattern, HTML error pages,
  // etc.) is forwarded as plain text with the backend's real status code —
  // instead of collapsing every non-JSON body into a generic "unexpected
  // response" message that hides the actual reason (e.g. "Username already
  // taken", "Username must be 3-20 characters").
  async fn forward_backend_response(resp: reqwest::Response) -> axum::response::Response {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    match resp.bytes().await {
      Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(v) => (status, Json(v)).into_response(),
        Err(_) => (status, String::from_utf8_lossy(&bytes).into_owned()).into_response(),
      },
      Err(e) => {
        tracing::warn!("[HTTP] failed to read backend response body: {e}");
        (StatusCode::BAD_GATEWAY, backend_bad_response_msg(e)).into_response()
      }
    }
  }

  async fn proxy_post(url: &str, body: serde_json::Value) -> axum::response::Response {
    let client = reqwest::Client::new();
    match client.post(url).json(&body).send().await {
      Ok(resp) => forward_backend_response(resp).await,
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
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
      Ok(resp) => forward_backend_response(resp).await,
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    }
  }

  // Auth proxy routes — capture JWT from responses so GET /token can serve it
  async fn api_login(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let resp = proxy_post(&format!("{}/api/auth/login", get_backend_url()), body).await;
    resp
  }
  async fn api_register(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/register", get_backend_url()), body).await
  }
  async fn api_login_email(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(&format!("{}/api/auth/login-email", get_backend_url()), body).await
  }
  async fn api_register_email(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/register-email", get_backend_url()),
      body,
    )
    .await
  }

  // POST /token — wallet-ui posts the JWT after successful auth so the game client can pick it up
  async fn post_token(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    if let Some(token) = body["token"].as_str() {
      *s.wallet_jwt.0.lock().unwrap() = Some(token.to_string());
      tracing::info!("[HTTP] JWT stored via /token");
    }
    StatusCode::OK
  }

  // GET /token — game client polls this to retrieve the JWT after wallet-ui auth
  async fn get_token(State(s): State<LocalState>) -> impl IntoResponse {
    let token = s.wallet_jwt.0.lock().unwrap().clone();
    Json(serde_json::json!({ "token": token }))
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
    proxy_post_auth(
      &format!("{}/api/auth/add-email", get_backend_url()),
      body,
      &headers,
    )
    .await
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
      Ok(resp) => forward_backend_response(resp).await,
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    }
  }
  async fn api_set_username(
    State(s): State<LocalState>,
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
        // The rename only actually took effect on the backend if it
        // succeeded — mirror it into our own in-memory cache too, so the
        // running game client's next GET /status sees it immediately
        // instead of the stale name it started this session with (that
        // used to require a full game restart to pick up).
        if resp.status().is_success() {
          if let Some(username) = body["username"].as_str() {
            if !username.is_empty() {
              *s.wallet_username.0.lock().unwrap() = Some(username.to_string());
            }
          }
        }
        forward_backend_response(resp).await
      }
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    }
  }

  // POST /api/auth/init-profile-tx — build unsigned initProfile tx (proxied with JWT)
  async fn api_init_profile_tx(
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post_auth(
      &format!("{}/api/auth/init-profile-tx", get_backend_url()),
      body,
      &headers,
    )
    .await
  }

  // POST /api/auth/broadcast-tx — broadcast a signed transaction (proxied)
  async fn api_broadcast_tx(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/broadcast-tx", get_backend_url()),
      body,
    )
    .await
  }

  // POST /api/open-profile-step — game client calls this when user tries to wager without
  // an on-chain profile. Sets a flag that the wallet-ui polls, and opens the popup.
  async fn api_open_profile_step(State(s): State<LocalState>) -> impl IntoResponse {
    s.needs_profile_step
      .store(true, std::sync::atomic::Ordering::Relaxed);
    let wallet_url =
      std::env::var("XFCHESS_WALLET_URL")
      .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
    let profile_url = format!("{wallet_url}?step=profile");
    tracing::info!("[HTTP] opening profile step: {profile_url}");
    tokio::task::spawn_blocking(move || {
      open_in_browser(&profile_url);
    });
    StatusCode::OK
  }

  // GET /api/needs-profile-step — wallet-ui polls this; returns true once then clears the flag.
  async fn api_needs_profile_step(State(s): State<LocalState>) -> impl IntoResponse {
    let needs = s
      .needs_profile_step
      .swap(false, std::sync::atomic::Ordering::Relaxed);
    Json(serde_json::json!({ "needs_profile": needs }))
  }

  // POST /api/game/launch — updates bridge-local username so the game sees it immediately
  // (the game polls GET /status, not this endpoint directly)
  async fn api_game_launch(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    // Update in-memory username so the next /status poll returns the final name.
    if let Some(username) = body["username"].as_str() {
      if !username.is_empty() {
        *s.wallet_username.0.lock().unwrap() = Some(username.to_string());
      }
    }
    StatusCode::OK
  }

  // Generic passthrough for remaining /api/** calls to backend
  async fn api_check_wallet(
    axum::extract::Path(pubkey): axum::extract::Path<String>,
  ) -> impl IntoResponse {
    let url = format!("{}/api/auth/check-wallet/{pubkey}", get_backend_url());
    match reqwest::get(&url).await {
      Ok(resp) => forward_backend_response(resp).await,
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    }
  }

  // Serve the tournament admin UI from the pre-built dist/. The admin panel is
  // desktop-only: it renders in the Tauri "tournament-admin" window, loaded from
  // this loopback-only bridge — there is no standalone vite/web dev server.
  // Rebuild the UI with: cd tauri/tournament-admin && npm run build
  //
  // Gated behind the `tournament-admin` cargo feature (off by default, and not
  // passed by release.yml) so a shipped consumer build has no route, window,
  // or IPC path capable of serving/opening the admin panel at all.
  #[cfg(feature = "tournament-admin")]
  async fn serve_tournament_admin(
    State(s): State<LocalState>,
    uri: axum::http::Uri,
  ) -> impl IntoResponse {
    serve_dist_file(&s.dist_path, "/tournament-admin", uri.path()).await
  }

  // Serves the wallet-ui SPA (built dist/) from the same loopback bridge the
  // popup already opens — always compiled in (unlike tournament-admin) since
  // every player needs wallet signing, not just desktop admins. Without this,
  // the popup has nowhere real to point at other than an external URL that
  // doesn't exist in a shipped build (see XFCHESS_WALLET_URL's default below).
  async fn serve_wallet_ui(
    State(s): State<LocalState>,
    uri: axum::http::Uri,
  ) -> impl IntoResponse {
    serve_dist_file(&s.wallet_ui_dist_path, "/wallet-ui", uri.path()).await
  }

  async fn serve_dist_file(
    dist: &std::path::Path,
    prefix: &str,
    url_path: &str,
  ) -> axum::response::Response {
    // Strip the mount prefix, treat the rest as a relative file path
    let rel = url_path
      .strip_prefix(prefix)
      .unwrap_or(url_path)
      .trim_start_matches('/')
      .split('?')
      .next()
      .unwrap_or(""); // drop query string

    // Route assets directly; everything else → index.html (SPA)
    let file_path = if rel.contains('.') {
      dist.join(rel)
    } else {
      dist.join("index.html")
    };

    let mime = match file_path.extension().and_then(|e| e.to_str()) {
      Some("html") => "text/html; charset=utf-8",
      Some("js") | Some("mjs") => "application/javascript",
      Some("css") => "text/css",
      Some("svg") => "image/svg+xml",
      Some("png") => "image/png",
      Some("ico") => "image/x-icon",
      Some("woff2") => "font/woff2",
      _ => "application/octet-stream",
    };

    match tokio::fs::read(&file_path).await {
      Ok(bytes) => axum::response::Response::builder()
        .header("Content-Type", mime)
        .body(axum::body::Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
      Err(_) => {
        // Try index.html as SPA fallback
        match tokio::fs::read(dist.join("index.html")).await {
          Ok(bytes) => axum::response::Response::builder()
            .header("Content-Type", "text/html; charset=utf-8")
            .body(axum::body::Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response()),
          Err(_) => (StatusCode::NOT_FOUND, format!("{prefix} dist not found. Build it first: cd tauri{prefix} && npm run build")).into_response(),
        }
      }
    }
  }

  // Only reflect local / Tauri-webview origins. This stops arbitrary websites the
  // user visits from reading bridge responses cross-origin (notably GET /token,
  // which would otherwise leak the wallet JWT to any page). The wallet-ui runs on
  // localhost (dev) or tauri.localhost (prod), so it stays allowed.
  let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin, _parts| {
      let o = origin.as_bytes();
      o.starts_with(b"tauri://")
        || o.starts_with(b"http://tauri.localhost")
        || o.starts_with(b"https://tauri.localhost")
        || o.starts_with(b"http://localhost:")
        || o.starts_with(b"http://127.0.0.1:")
    }))
    .allow_methods([
      Method::GET,
      Method::POST,
      axum::http::Method::PATCH,
      axum::http::Method::DELETE,
      axum::http::Method::OPTIONS,
    ])
    .allow_headers(Any);

  // Resolve the tournament-admin dist dir:
  // 1. Next to the binary (production bundle copies it there)
  // 2. CARGO_MANIFEST_DIR-relative (dev: workspace/tauri/tournament-admin/dist)
  #[cfg(feature = "tournament-admin")]
  let dist_path = {
    let dev_path = std::path::PathBuf::from(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/tournament-admin/dist"
    ));
    if dev_path.exists() {
      dev_path
    } else {
      std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("tournament-admin/dist")))
        .unwrap_or(dev_path)
    }
  };

  // Resolve the wallet-ui dist dir the same way: next to the binary in a
  // production bundle, or CARGO_MANIFEST_DIR-relative in dev.
  let wallet_ui_dist_path = {
    let dev_path =
      std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/wallet-ui/dist"));
    if dev_path.exists() {
      dev_path
    } else {
      std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("wallet-ui/dist")))
        .unwrap_or(dev_path)
    }
  };

  let state = LocalState {
    app,
    pending,
    notify,
    wallet_pubkey,
    wallet_username,
    wallet_jwt,
    #[cfg(feature = "tournament-admin")]
    dist_path,
    wallet_ui_dist_path,
    needs_profile_step: Arc::new(std::sync::atomic::AtomicBool::new(false)),
  };

  let router = Router::new()
    .route("/pending", get(get_pending))
    .route("/pending/stream", get(get_pending_stream))
    .route("/resolved", post(post_resolved))
    .route("/wallet", post(post_wallet))
    .route("/hide", post(post_hide))
    .route("/status", get(get_status))
    .route("/token", get(get_token).post(post_token))
    .route("/wallet-ui", get(serve_wallet_ui))
    .route("/wallet-ui/", get(serve_wallet_ui))
    .route("/wallet-ui/{*path}", get(serve_wallet_ui))
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
    .route("/api/auth/init-profile-tx", post(api_init_profile_tx))
    .route("/api/auth/broadcast-tx", post(api_broadcast_tx))
    .route("/api/game/launch", post(api_game_launch))
    .route("/api/open-profile-step", post(api_open_profile_step))
    .route("/api/needs-profile-step", get(api_needs_profile_step));

  // Tournament admin UI (built dist, rendered in the desktop admin window).
  // Only wired up when compiled with --features tournament-admin — a default
  // build (what release.yml ships) has no route capable of serving it.
  #[cfg(feature = "tournament-admin")]
  let router = router
    .route(
      "/tournament-admin",
      axum::routing::get(serve_tournament_admin),
    )
    .route(
      "/tournament-admin/",
      axum::routing::get(serve_tournament_admin),
    )
    .route(
      "/tournament-admin/{*path}",
      axum::routing::get(serve_tournament_admin),
    );

  let router = router.layer(cors).with_state(state);

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
  open_wallet_popup_with_step(None);
}

/// Open the wallet UI to approve a pending transaction. Passing `?step=sign`
/// tells wallet-ui (see hasExistingSession in App.tsx) to skip straight past
/// the login/profile walkthrough when a session is already on disk — a plain
/// `open_wallet_popup()` reopens the base URL, which always restarts at
/// consent/entry, so a signing request that arrives after the user already
/// logged in used to show a fresh "log in again" screen instead of the sign
/// prompt, and the pending tx would silently time out 60s later.
fn open_wallet_popup_for_signing(_app: &tauri::AppHandle) {
  open_wallet_popup_with_step(Some("sign"));
}

fn open_wallet_popup_with_step(step: Option<&str>) {
  let wallet_url =
    std::env::var("XFCHESS_WALLET_URL")
      .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
  let url = match step {
    Some(s) => format!("{wallet_url}?step={s}"),
    None => wallet_url,
  };
  tracing::info!("[WalletPopup] opening in system browser: {url}");
  open_in_browser(&url);
}

/// Open a URL in Chrome app-mode (compact popup, no address bar).
/// Falls back to the system default browser if Chrome is not found.
/// PID of the last Chrome process spawned for the wallet popup, so `/hide`
/// can actually close it — `window.close()` from inside the popup is
/// unreliable since Chrome treats a CLI-launched `--app` window as not
/// script-opened and blocks it.
fn wallet_popup_pid_cell() -> &'static std::sync::Mutex<Option<u32>> {
  static CELL: std::sync::OnceLock<std::sync::Mutex<Option<u32>>> = std::sync::OnceLock::new();
  CELL.get_or_init(|| std::sync::Mutex::new(None))
}

fn open_in_browser(url: &str) {
  let ts = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let sep = if url.contains('?') { '&' } else { '?' };
  let url_ts = format!("{url}{sep}_t={ts}");

  #[cfg(windows)]
  {
    // If a popup window with title XFChess #<port> is already open or hidden,
    // bring it to front instantly instead of spawning a new browser process.
    if show_and_foreground_wallet_popup() {
      tracing::debug!("[WalletPopup] reused existing popup window");
      return;
    }

    // Chrome's per-user (non-admin) installer puts the binary under
    // %LOCALAPPDATA% instead of Program Files — that's actually the more
    // common layout on end-user machines than the two Program Files paths
    // below, so check it first. Fall back to Edge (bundled with every
    // Windows 10/11 install) in --app mode before giving up on a compact
    // popup entirely — without one of these, a Chrome-less machine falls
    // through to `open::that`, which opens a normal maximized browser tab
    // instead of the small popup this UI is designed for.
    let local_appdata_chrome = std::env::var("LOCALAPPDATA")
      .ok()
      .map(|v| format!(r"{v}\Google\Chrome\Application\chrome.exe"));
    let mut browser_paths: Vec<&str> = Vec::new();
    if let Some(ref p) = local_appdata_chrome {
      browser_paths.push(p.as_str());
    }
    browser_paths.push(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
    browser_paths.push(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe");
    browser_paths.push(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
    browser_paths.push(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe");

    let app_flag = format!("--app={}", url_ts);
    for path in &browser_paths {
      if std::path::Path::new(path).exists() {
        match Command::new(path)
          .args([&app_flag, "--window-size=460,720"])
          .spawn()
        {
          Ok(child) => {
            let pid = child.id();
            *wallet_popup_pid_cell().lock().unwrap() = Some(pid);
            // Deliberately NOT force-focusing the popup here (that used to
            // call force_foreground_window(pid)). Stealing OS foreground
            // focus from the game meant the *next* click back on the game
            // window — e.g. Cancel on the "Connect Wallet" overlay — was
            // consumed by Windows just to refocus it instead of reaching
            // Bevy/egui, making the overlay look broken/unclickable. The
            // popup still opens and Windows flashes its taskbar icon on its
            // own; the user can switch to it when it actually needs a click
            // (most reconnects now resolve silently via `onlyIfTrusted` and
            // never need one at all — see WalletStep's autoConnect).
          }
          Err(e) => tracing::warn!("[WalletPopup] failed to spawn {path}: {e}"),
        }
        return;
      }
    }
    // Last resort: default browser (cross-platform via the `open` crate).
    // This opens a normal, non-compact browser window/tab — expected only
    // when neither Chrome nor Edge is found at any known path.
    tracing::warn!("[WalletPopup] no Chrome/Edge found at known paths — falling back to default browser (will not be a compact popup)");
    let _ = open::that(&url_ts);
  }
  #[cfg(not(windows))]
  {
    let _ = open::that(&url_ts);
  }
}

/// Close the wallet-popup window.
///
/// This does NOT use the PID `open_in_browser` recorded from `Command::spawn`.
/// Chrome (and Edge) enforce one process per user-data-dir: if the user
/// already has a browser window open — normal for almost everyone — our
/// `--app=` invocation just forwards the request to that *already-running*
/// process via Chrome's own single-instance IPC and immediately exits, so
/// the spawned PID we tracked belongs to a process that's already dead by
/// the time this runs. `TerminateProcess` on it is therefore a silent no-op,
/// which is exactly the "Continue doesn't close the window" bug.
///
/// Deliberately not using an isolated `--user-data-dir` to sidestep that —
/// the wallet popup depends on the user's real Chrome profile for the
/// Phantom/Solflare extensions it talks to via `window.phantom`/`window.solflare`;
/// a fresh profile would have neither installed.
///
/// Instead: find the actual top-level window by title (the page is titled
/// `XFChess #<port>` — see `tauri/wallet-ui/src/App.tsx`, which stamps its
/// own bridge port onto `document.title` at load) whose owning process is
/// chrome.exe/msedge.exe (never the main game, which is a native window
/// under `xfchess.exe`, so this can't ever match the wrong "XFChess"-titled
/// window), and post it a real `WM_CLOSE`.
///
/// The port suffix matters: with a bare "XFChess" title, two local
/// instances (e.g. `just dev2`'s P1 on port 7454 and P2 on port 7464) are
/// indistinguishable to `EnumWindows`, which searches the whole desktop —
/// either player's popup closing would `WM_CLOSE` *both* players' popups.
#[cfg(windows)]
fn kill_wallet_popup() {
  use ::windows::core::BOOL;
  use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
  use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
  };
  use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
  };

  let expected_title = format!("XFChess #{}", http_port());

  extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
      let expected_title = &*(lparam.0 as *const String);

      let mut title_buf = [0u16; 256];
      let len = GetWindowTextW(hwnd, &mut title_buf);
      if len <= 0 {
        return BOOL(1); // keep enumerating
      }
      if String::from_utf16_lossy(&title_buf[..len as usize]) != *expected_title {
        return BOOL(1);
      }

      let mut pid: u32 = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut pid));
      if pid == 0 {
        return BOOL(1);
      }

      if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
        let mut name_buf = [0u16; 260];
        let mut size = name_buf.len() as u32;
        let queried = QueryFullProcessImageNameW(
          handle,
          PROCESS_NAME_WIN32,
          ::windows::core::PWSTR(name_buf.as_mut_ptr()),
          &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if queried {
          let path = String::from_utf16_lossy(&name_buf[..size as usize]).to_lowercase();
          if path.ends_with("chrome.exe") || path.ends_with("msedge.exe") {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            tracing::info!("[WalletPopup] closed popup window (hwnd owned by {path})");
          }
        }
      }
      BOOL(1) // a stray unrelated "XFChess"-titled window shouldn't stop the search
    }
  }

  unsafe {
    let _ = EnumWindows(
      Some(enum_proc),
      LPARAM(&expected_title as *const String as isize),
    );
  }
}

#[cfg(not(windows))]
fn kill_wallet_popup() {}

/// Timestamp of the last time the popup was hidden (not killed). `None`
/// means "not currently hidden" — either it was never opened, it's showing,
/// or it was actually killed. Read by the idle reaper (below) to decide when
/// a long-forgotten hidden popup should finally be killed for real, and
/// cleared whenever the popup is shown again or killed.
fn wallet_popup_hidden_at_cell() -> &'static std::sync::Mutex<Option<std::time::Instant>> {
  static CELL: std::sync::OnceLock<std::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::OnceLock::new();
  CELL.get_or_init(|| std::sync::Mutex::new(None))
}

/// Hide (not close) the wallet popup after a signature resolves, so the next
/// signing request can reuse the same already-warm Chrome process/page
/// instead of paying a full spawn + extension-reconnect cost. Same
/// EnumWindows/title/owning-process match as `kill_wallet_popup` — see its
/// doc comment for why title match (not the tracked spawn PID) is required.
/// Falls back to a real kill if no matching window is found, so a bug here
/// never leaves the wallet stuck (no popup, but the game keeps waiting).
#[cfg(windows)]
fn hide_wallet_popup() {
  use ::windows::core::BOOL;
  use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
  use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
  };
  use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, ShowWindow, SW_HIDE,
  };

  let expected_title = format!("XFChess #{}", http_port());
  let found = std::sync::atomic::AtomicBool::new(false);

  extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
      let ctx = &*(lparam.0 as *const (String, &std::sync::atomic::AtomicBool));
      let (expected_title, found) = ctx;

      let mut title_buf = [0u16; 256];
      let len = GetWindowTextW(hwnd, &mut title_buf);
      if len <= 0 {
        return BOOL(1);
      }
      if String::from_utf16_lossy(&title_buf[..len as usize]) != *expected_title {
        return BOOL(1);
      }

      let mut pid: u32 = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut pid));
      if pid == 0 {
        return BOOL(1);
      }

      if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
        let mut name_buf = [0u16; 260];
        let mut size = name_buf.len() as u32;
        let queried = QueryFullProcessImageNameW(
          handle,
          PROCESS_NAME_WIN32,
          ::windows::core::PWSTR(name_buf.as_mut_ptr()),
          &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if queried {
          let path = String::from_utf16_lossy(&name_buf[..size as usize]).to_lowercase();
          if path.ends_with("chrome.exe") || path.ends_with("msedge.exe") {
            let _ = ShowWindow(hwnd, SW_HIDE);
            found.store(true, std::sync::atomic::Ordering::SeqCst);
            tracing::info!("[WalletPopup] hid popup window (hwnd owned by {path})");
          }
        }
      }
      BOOL(1)
    }
  }

  let ctx = (expected_title, &found);
  unsafe {
    let _ = EnumWindows(Some(enum_proc), LPARAM(&ctx as *const _ as isize));
  }

  if found.load(std::sync::atomic::Ordering::SeqCst) {
    *wallet_popup_hidden_at_cell().lock().unwrap() = Some(std::time::Instant::now());
  } else {
    // No matching window — nothing to hide, nothing to reap later either.
    tracing::debug!("[WalletPopup] hide requested but no popup window found");
    *wallet_popup_hidden_at_cell().lock().unwrap() = None;
  }
}

#[cfg(not(windows))]
fn hide_wallet_popup() {
  kill_wallet_popup();
}

/// Unhide and foreground a popup window previously hidden by
/// `hide_wallet_popup`, so a *new* signing request reuses it instead of
/// `open_in_browser` spawning a fresh Chrome process. Same title/owning-
/// process match as `hide_wallet_popup`/`kill_wallet_popup` — deliberately
/// not `force_foreground_window`'s PID+`IsWindowVisible` match, since a
/// hidden window fails `IsWindowVisible` by definition.
#[cfg(windows)]
fn show_and_foreground_wallet_popup() -> bool {
  use ::windows::core::BOOL;
  use ::windows::Win32::Foundation::{HWND, LPARAM};
  use ::windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
  use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    SetForegroundWindow, ShowWindow, SW_SHOW,
  };

  let expected_title = format!("XFChess #{}", http_port());

  extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
      let ctx = &mut *(lparam.0 as *mut (String, HWND));
      let mut title_buf = [0u16; 256];
      let len = GetWindowTextW(hwnd, &mut title_buf);
      if len <= 0 || String::from_utf16_lossy(&title_buf[..len as usize]) != ctx.0 {
        return BOOL(1);
      }
      ctx.1 = hwnd;
      BOOL(0)
    }
  }

  let mut ctx: (String, HWND) = (expected_title, HWND(std::ptr::null_mut()));
  unsafe {
    let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
  }

  if ctx.1 .0.is_null() {
    return false;
  }

  unsafe {
    let target = ctx.1;
    let _ = ShowWindow(target, SW_SHOW);
    let foreground = GetForegroundWindow();
    let foreground_tid = GetWindowThreadProcessId(foreground, None);
    let current_tid = GetCurrentThreadId();
    let _ = AttachThreadInput(current_tid, foreground_tid, true);
    let _ = SetForegroundWindow(target);
    let _ = AttachThreadInput(current_tid, foreground_tid, false);
  }
  *wallet_popup_hidden_at_cell().lock().unwrap() = None;
  true
}

#[cfg(not(windows))]
fn show_and_foreground_wallet_popup() -> bool {
  false
}

/// How long a hidden popup can sit idle before it's actually killed, so a
/// player who finishes their session doesn't leave a wallet-extension-
/// capable Chrome window running invisibly in the background forever.
const HIDDEN_POPUP_IDLE_KILL_SECS: u64 = 15 * 60;

/// Background task, run for the app's lifetime: periodically checks whether
/// the popup has been hidden (not killed, see `hide_wallet_popup`) for
/// longer than `HIDDEN_POPUP_IDLE_KILL_SECS` and, if so, kills it for real.
fn spawn_wallet_popup_idle_reaper() {
  tauri::async_runtime::spawn(async move {
    loop {
      tokio::time::sleep(std::time::Duration::from_secs(60)).await;
      let should_kill = wallet_popup_hidden_at_cell()
        .lock()
        .unwrap()
        .is_some_and(|at| at.elapsed().as_secs() >= HIDDEN_POPUP_IDLE_KILL_SECS);
      if should_kill {
        tracing::info!(
          "[WalletPopup] hidden popup idle for {HIDDEN_POPUP_IDLE_KILL_SECS}s — killing for real"
        );
        kill_wallet_popup();
        *wallet_popup_hidden_at_cell().lock().unwrap() = None;
      }
    }
  });
}

/// Whether a process with this PID is still running. Used to decide whether
/// a previously-spawned wallet popup can be refocused instead of duplicated.
#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
  use ::windows::Win32::Foundation::CloseHandle;
  use ::windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
  };

  unsafe {
    let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
      return false;
    };
    let mut exit_code: u32 = 0;
    let alive = GetExitCodeProcess(handle, &mut exit_code).is_ok()
      && exit_code == ::windows::Win32::Foundation::STILL_ACTIVE.0 as u32;
    let _ = CloseHandle(handle);
    alive
  }
}


#[tauri::command]
fn show_wallet_popup_window(app: tauri::AppHandle) {
  tracing::info!("[WalletPopup] show_wallet_popup_window invoked");
  open_wallet_popup(&app);
}

// Gated behind the `tournament-admin` cargo feature (off by default; not
// passed by release.yml) — a shipped consumer build gets the no-op fallback
// below instead, so there's no code path that can ever create this window.
#[cfg(feature = "tournament-admin")]
fn open_tournament_admin(app: &tauri::AppHandle) {
  // Window creation MUST run on the main thread in Tauri v2.
  let app2 = app.clone();
  let _ = app.run_on_main_thread(move || {
    let app = app2;
    // Served by the loopback-only wallet bridge from the built dist — the
    // admin panel only exists inside this desktop window, never as a
    // separate web process.
    let admin_url = format!("http://localhost:{}/tournament-admin/", http_port());
    if let Some(win) = app.get_webview_window("tournament-admin") {
      tracing::info!("[TournamentAdmin] focusing existing window");
      let _ = win.show();
      let _ = win.set_focus();
    } else {
      tracing::info!("[TournamentAdmin] creating window → {admin_url}");
      let url = tauri::WebviewUrl::External(admin_url.parse().expect("valid URL"));
      match tauri::WebviewWindowBuilder::new(&app, "tournament-admin", url)
        .title("XFChess Tournament Admin")
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .resizable(true)
        .decorations(false)
        .shadow(true)
        .center()
        .build()
      {
        Ok(win) => {
          let _ = win.show();
          let _ = win.set_focus();
        }
        Err(e) => tracing::error!("[TournamentAdmin] failed to create window: {e}"),
      }
    }
  });
}

#[cfg(not(feature = "tournament-admin"))]
fn open_tournament_admin(_app: &tauri::AppHandle) {
  tracing::warn!(
    "[TournamentAdmin] admin panel is not compiled into this build (needs --features tournament-admin)"
  );
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
      let wallet_jwt = WalletJwt::default();
      let pending_tx: PendingTx = Arc::new(Mutex::new(None));
      let (pending_notify, _): (PendingTxNotify, _) = tokio::sync::watch::channel(());
      let auth_state = services::auth::AuthState::new();

      // Register shared state with Tauri app
      app.manage(wallet_pubkey.clone());
      app.manage(wallet_username.clone());
      app.manage(wallet_jwt.clone());
      app.manage(pending_tx.clone());
      app.manage(auth_state);

      // ── HTTP wallet bridge — /pending, /pending/stream, /resolved, /wallet,
      // /hide, /token ── The wallet-ui React app subscribes to
      // http://localhost:7454/pending/stream (SSE) for unsigned transactions
      // and posts signed results back. GET /token lets the game client
      // retrieve the JWT issued during wallet-ui auth.
      {
        let h = app.handle().clone();
        let p = pending_tx.clone();
        let n = pending_notify.clone();
        let w = wallet_pubkey.clone();
        let wu = wallet_username.clone();
        let wj = wallet_jwt.clone();
        tauri::async_runtime::spawn(http_server(h, p, n, w, wu, wj));
      }

      spawn_wallet_popup_idle_reaper();

      // Initialize windows
      #[cfg(feature = "tournament-admin")]
      {
        let _ = TournamentAdminWindow::new(app.handle());
      }

      // ── Tournament admin auto-open (just dev / just admin / start-tournament-admin.bat) ──
      // Retries until the window exists: on a cold start the event loop may not
      // be able to create windows yet, and a single delayed attempt gets dropped.
      // Feature-gated: a default/release build never even checks the env var.
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

      // ── Wallet Bridge TCP listener ──────────────────────────────────────────
      // Two things arrive on this socket:
      //   - the literal 4 bytes "OPEN", from open_wallet_browser() — just a
      //     "show the popup" ping.
      //   - a label-and-length-prefixed transaction from
      //     tauri_signer::send_to_tauri_blocking
      //     ([4-byte LE label length][label utf8][4-byte LE tx length][tx bytes])
      //     — a real signing request, which this listener must hand off to
      //     wallet-ui (via the existing /pending + /resolved HTTP bridge, same
      //     PendingTx the axum server uses) and block on, then write
      //     [4-byte LE length][signed bytes] back — or the 0xFFFFFFFF sentinel
      //     Bevy already treats as "rejected".
      {
        let app_handle = app.handle().clone();
        let pending_for_tcp = pending_tx.clone();
        let notify_for_tcp = pending_notify.clone();
        let wallet_pubkey_for_tcp = wallet_pubkey.clone();
        let base_port: u16 = std::env::var("XFCHESS_WALLET_PORT")
          .ok()
          .and_then(|v| v.parse().ok())
          .unwrap_or(7454);
        std::thread::spawn(move || {
          let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("[WalletBridge] tokio runtime");
          rt.block_on(async move {
            // Try binding on ports base-11 through base-2, lowest first (must
            // match the client's scan order in tcp_port_range() so a cold
            // client's fallback scan — used only if the port-file handshake
            // below is unavailable — finds us on its first live attempt
            // instead of walking past an unrelated listener on another port
            // in range, which can itself be a live HTTP/TCP server that eats
            // several seconds before closing the connection).
            let mut listener = None;
            let mut bound_port: u16 = 0;
            for offset in (2u16..=11).rev() {
              let port = base_port.saturating_sub(offset);
              if let Ok(l) = TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                tracing::info!("[WalletBridge] Listening on port {}", port);
                listener = Some(l);
                bound_port = port;
                break;
              }
            }
            let listener = match listener {
              Some(l) => l,
              None => {
                tracing::warn!("[WalletBridge] No port available");
                return;
              }
            };

            // Announce the actual bound port so the client can connect
            // directly instead of scanning — the scan is a fallback only.
            let port_file = wallet_bridge_port_file(base_port);
            if let Err(e) = std::fs::write(&port_file, bound_port.to_string()) {
              tracing::warn!(
                "[WalletBridge] failed to write port file {port_file:?}: {e}"
              );
            }
            loop {
              if let Ok((mut stream, _)) = listener.accept().await {
                let app2 = app_handle.clone();
                let pending2 = pending_for_tcp.clone();
                let notify2 = notify_for_tcp.clone();
                let wallet_pubkey2 = wallet_pubkey_for_tcp.clone();
                tokio::spawn(async move {
                  let mut prefix = [0u8; 4];
                  if stream.read_exact(&mut prefix).await.is_err() {
                    return;
                  }

                  if &prefix == b"OPEN" {
                    open_wallet_popup(&app2);
                    return;
                  }

                  // `query_wallet_pubkey_from_tauri()` in the game client
                  // (src/multiplayer/solana/integration/systems.rs) polls
                  // this every few seconds — respond with whatever `/wallet`
                  // last recorded, or a 0-length response (client reads
                  // that as "not connected") rather than falling through to
                  // the signing-protocol parse below, which used to
                  // misread these 4 bytes as an implausible label length
                  // and reject/warn on every single poll.
                  if &prefix == b"PKEY" {
                    let pk = wallet_pubkey2.0.lock().unwrap().clone().unwrap_or_default();
                    let pk_bytes = pk.into_bytes();
                    let len_bytes = (pk_bytes.len() as u32).to_le_bytes();
                    let _ = stream.write_all(&len_bytes).await;
                    if !pk_bytes.is_empty() {
                      let _ = stream.write_all(&pk_bytes).await;
                    }
                    return;
                  }

                  // Otherwise `prefix` is a little-endian u32 byte length for
                  // the label, followed by the label itself, then the tx.
                  const MAX_LABEL_LEN: usize = 256;
                  const MAX_TX_LEN: usize = 64 * 1024; // real txs are a few KB
                  let label_len = u32::from_le_bytes(prefix) as usize;
                  if label_len > MAX_LABEL_LEN {
                    tracing::warn!(
                      "[WalletBridge] rejecting signing request with implausible label length {label_len}"
                    );
                    return;
                  }
                  let mut label_bytes = vec![0u8; label_len];
                  if stream.read_exact(&mut label_bytes).await.is_err() {
                    tracing::warn!("[WalletBridge] failed to read label");
                    return;
                  }
                  let label = String::from_utf8_lossy(&label_bytes).into_owned();

                  let mut tx_len_buf = [0u8; 4];
                  if stream.read_exact(&mut tx_len_buf).await.is_err() {
                    tracing::warn!("[WalletBridge] failed to read tx length");
                    return;
                  }
                  let len = u32::from_le_bytes(tx_len_buf) as usize;
                  if len == 0 || len > MAX_TX_LEN {
                    tracing::warn!(
                      "[WalletBridge] rejecting signing request with implausible length {len}"
                    );
                    return;
                  }
                  let mut tx_bytes = vec![0u8; len];
                  if stream.read_exact(&mut tx_bytes).await.is_err() {
                    tracing::warn!("[WalletBridge] failed to read full tx payload");
                    return;
                  }

                  let (resp_tx, resp_rx) = oneshot::channel();
                  {
                    let mut guard = pending2.lock().unwrap();
                    *guard = Some((tx_bytes, label, resp_tx));
                  }
                  let _ = notify2.send(());
                  // Ensure the popup is open/focused so the user can approve.
                  // Dedup-guarded on the Rust side (see process_is_alive in
                  // open_in_browser), so this is a no-op if one is already up.
                  open_wallet_popup_for_signing(&app2);

                  let outcome = tokio::time::timeout(
                    std::time::Duration::from_secs(SIGN_TIMEOUT_SECS),
                    resp_rx,
                  )
                  .await;

                  match outcome {
                    Ok(Ok(Ok(signed_bytes))) => {
                      let len_bytes = (signed_bytes.len() as u32).to_le_bytes();
                      let _ = stream.write_all(&len_bytes).await;
                      let _ = stream.write_all(&signed_bytes).await;
                    }
                    other => {
                      if let Err(e) = &other {
                        tracing::warn!("[WalletBridge] signing timed out: {e}");
                      } else if let Ok(Ok(Err(e))) = &other {
                        tracing::info!("[WalletBridge] signing rejected: {e}");
                      }
                      // Clear a stale pending entry left by a timeout — a
                      // real /resolved call already takes() it, so this is a
                      // no-op in that case.
                      *pending2.lock().unwrap() = None;
                      let _ = notify2.send(());
                      let _ = stream.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                    }
                  }
                });
              }
            }
          });
        });
      }

      // ── Background Notification Poller ──────────────────────────────────────
      // Same prod-by-default rule as get_backend_url() above — must not
      // default anywhere the game client and wallet-bridge proxy don't.
      let backend_url = std::env::var("VITE_BACKEND_URL")
        .or_else(|_| std::env::var("SIGNING_SERVICE_URL"))
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| "https://xfchess.com".to_string());
      services::notification_poller::start_poller(
        app.handle().clone(),
        backend_url,
        wallet_pubkey.0.clone(),
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
    ])
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|_app_handle, event| {
      // The wallet popup is now hidden-not-killed between signatures (see
      // hide_wallet_popup) so it can be reused instead of respawned — kill
      // it for real on app exit so it never lingers as an invisible,
      // wallet-extension-capable Chrome process after the game closes.
      if let tauri::RunEvent::ExitRequested { .. } = event {
        kill_wallet_popup();
      }
    });
}
