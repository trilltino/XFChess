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
#[cfg(windows)]
use std::os::windows::process::CommandExt;
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

/// Which provider the connected wallet came from: `phantom`, `solflare`, or
/// `privy`. Reported by wallet-ui on `POST /wallet` and surfaced on `/status`.
///
/// The game client uses this to decide whether the no-popup global-session flow
/// is safe to attempt: it is enabled only for `privy` embedded wallets, because
/// the one unresolved blocker on that flow (a Solflare "network mismatch:
/// current network devnet, but this transaction is for mainnet" rejection
/// arriving with no user action) is an artifact of an extension carrying its own
/// user-selected cluster. An embedded wallet has no such setting and cannot
/// produce it. See `authorize_global_session_if_needed` in
/// src/multiplayer/solana/integration/systems.rs.
#[derive(Default, Clone)]
struct WalletProvider(Arc<Mutex<Option<String>>>);

/// JWT token issued by the backend on successful auth.
/// Shared between the bridge HTTP server and the main app handle.
#[derive(Default, Clone)]
struct WalletJwt(Arc<Mutex<Option<String>>>);

/// When the game client last polled `GET /status`. This bridge process
/// survives independently of the game window (closing the game doesn't kill
/// it — see `spawn_wallet_state_reaper`'s doc comment for why that used to
/// mean a connected wallet stayed cached here forever, across every later
/// launch, until the whole process was manually killed). Used only to detect
/// "no game has been alive/polling for a while" so the cached wallet can be
/// dropped on that basis, without needing to track the game's actual PID.
#[derive(Default, Clone)]
struct WalletLastSeen(Arc<Mutex<Option<std::time::Instant>>>);

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

/// The port the axum HTTP server actually bound to, once it has. Two
/// instances on one machine with no XFCHESS_WALLET_PORT override both try
/// the same nominal port; the second one's bind fails outright (logged, not
/// fatal), leaving that whole process running with no HTTP server at all —
/// every URL built from the nominal port (the wallet-signing popup, /status
/// polling, wallet-ui itself) then points at nothing. Set once bind_http_port
/// finds a free port; every other caller of http_port() picks it up
/// transparently instead of trusting the nominal value blindly.
static ACTUAL_HTTP_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

fn nominal_http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

/// Get the HTTP port for the wallet signing service — the real bound port
/// if the server has started, else the nominal (env-derived) one.
fn http_port() -> u16 {
  ACTUAL_HTTP_PORT
    .get()
    .copied()
    .unwrap_or_else(nominal_http_port)
}

/// Path the HTTP bridge writes its actual bound port to, so the game client
/// (a separate process) can discover it instead of assuming the nominal
/// value always matches. Mirrors wallet_bridge_port_file() below for the
/// raw-TCP listener; must match http_bridge_port_file() in the game
/// client's src/multiplayer/solana/tauri_signer.rs.
fn http_bridge_port_file() -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-http-{}.port", nominal_http_port()))
}

/// Bind the HTTP server, trying the nominal port first and then a small
/// range above it if that's taken (another instance already bound it).
/// Writes whichever port actually worked to http_bridge_port_file() and
/// records it in ACTUAL_HTTP_PORT before returning.
async fn bind_http_port() -> Option<(TcpListener, u16)> {
  let nominal = nominal_http_port();
  for port in std::iter::once(nominal).chain(nominal.saturating_add(1)..=nominal.saturating_add(10))
  {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if let Ok(listener) = TcpListener::bind(addr).await {
      let _ = ACTUAL_HTTP_PORT.set(port);
      std::env::set_var("XFCHESS_ACTUAL_WALLET_PORT", port.to_string());
      let _ = std::fs::write(http_bridge_port_file(), port.to_string());
      return Some((listener, port));
    }
  }
  None
}

/// Path used to announce the raw-TCP wallet bridge's actual bound port to
/// the game client, keyed by `base_port` (XFCHESS_WALLET_PORT) so multiple
/// local instances (e.g. `just dev2`'s P1/P2) never collide on one file.
/// Must match `wallet_bridge_port_file()` in the game client's
/// `src/multiplayer/solana/tauri_signer.rs`.
fn wallet_bridge_port_file(base_port: u16) -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-bridge-{base_port}.port"))
}

/// Backend URL the game client (a separate process) has explicitly told us
/// it resolved, via `POST /api/set-backend-url` — see `open_wallet_browser()`
/// in `src/multiplayer/solana/tauri_signer.rs`. Preferred over independently
/// re-deriving the same env-var precedence here, which is exactly what let
/// the two processes silently disagree (see `get_backend_url`'s doc comment)
/// if only one of them had `SIGNING_SERVICE_URL`/`BACKEND_URL` set in its
/// environment.
static BACKEND_URL_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
  std::sync::OnceLock::new();

fn backend_url_override_cell() -> &'static std::sync::Mutex<Option<String>> {
  BACKEND_URL_OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Get the backend API base URL.
///
/// Resolution order:
/// 1. Explicit override from the game client (`set_backend_url_override`).
/// 2. `SIGNING_SERVICE_URL` / `BACKEND_URL` env vars — same order as the game
///    client's own fallback in `vps_base()` (`src/multiplayer/network/vps/client.rs`),
///    used when this process never heard from a game client (e.g. the popup
///    was opened standalone, or an older game client build that predates
///    `set-backend-url`).
/// 3. Production Hetzner backend.
///
/// (1) and (2) MUST stay in the same order/precedence as the game client's
/// own resolution — if they ever diverge, `/api/auth/*` calls proxied
/// through this bridge silently 502 while the game client's own VPS calls
/// succeed, a confusing split-brain failure that looks like a server outage
/// but isn't.
fn get_backend_url() -> String {
  if let Some(url) = backend_url_override_cell().lock().unwrap().clone() {
    return url;
  }
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "https://xfchess.com".to_string())
}

/// Record the backend URL the game client says it resolved. Logged loudly
/// on every change (not just once) so a mismatch between two locally-running
/// instances, or a stale override from a previous session's game client,
/// is visible in the bridge's own log instead of only manifesting as
/// confusing 502s downstream.
fn set_backend_url_override(url: String) {
  let mut cell = backend_url_override_cell().lock().unwrap();
  let changed = cell.as_deref() != Some(url.as_str());
  if changed {
    tracing::info!("[Backend] target set by game client: {url}");
  }
  *cell = Some(url);
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
  wallet_provider: WalletProvider,
  wallet_jwt: WalletJwt,
  wallet_last_seen: WalletLastSeen,
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
    wallet_provider: WalletProvider,
    wallet_jwt: WalletJwt,
    wallet_last_seen: WalletLastSeen,
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
  //
  // A present-but-empty `username` is a deliberate, explicit "this wallet is
  // confirmed to have no username yet" — distinct from the field being
  // absent, which means the caller has no opinion and the existing cached
  // value (if any) should be left alone. Collapsing those two cases (as the
  // old `if !username.is_empty()` guard did) meant an empty string was
  // silently ignored, so a stale username left over from an entirely
  // different wallet's earlier session in this same shared browser profile
  // could never be cleared — the game client's poller kept showing it as
  // "already known" for whichever wallet connected next, even after
  // wallet-ui itself correctly determined there was no real username for
  // that wallet (see WalletStep.handleConnect and handleAuth in App.tsx for
  // the two call sites this now correctly distinguishes between).
  async fn post_wallet(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    if let Some(pk) = body["pubkey"].as_str() {
      *s.wallet_pubkey.0.lock().unwrap() = Some(pk.to_string());
      if let Some(username) = body.get("username").and_then(|v| v.as_str()) {
        *s.wallet_username.0.lock().unwrap() = if username.is_empty() {
          None
        } else {
          Some(username.to_string())
        };
      }
      // Absent means "caller has no opinion" — leave whatever is cached alone,
      // same convention as `username` above.
      if let Some(provider) = body.get("provider").and_then(|v| v.as_str()) {
        *s.wallet_provider.0.lock().unwrap() = if provider.is_empty() {
          None
        } else {
          Some(provider.to_string())
        };
      }
      tracing::info!(
        "[HTTP] Wallet connected: {pk} username={}",
        body
          .get("username")
          .and_then(|v| v.as_str())
          .unwrap_or("<unset>")
      );
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

  // GET /status — health / wallet info. Only the game client polls this
  // (every 5s while running — see `poll_wallet_bridge` in
  // src/states/main_menu.rs), so a call here doubles as "the game is alive
  // right now" for `spawn_wallet_state_reaper` below.
  async fn get_status(State(s): State<LocalState>) -> impl IntoResponse {
    *s.wallet_last_seen.0.lock().unwrap() = Some(std::time::Instant::now());
    let pubkey = s.wallet_pubkey.0.lock().unwrap().clone();
    let username = s.wallet_username.0.lock().unwrap().clone();
    let provider = s.wallet_provider.0.lock().unwrap().clone();
    Json(serde_json::json!({
      "connected": pubkey.is_some(),
      "pubkey": pubkey,
      "username": username,
      "provider": provider,
    }))
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

  // Forwards the two client headers that matter for a proxied backend call:
  // - Authorization, so JWT-gated backend routes still see the caller's
  //   token.
  // - X-Session-Id (set by wallet-ui from the `sid` it read off its own
  //   popup URL, see App.tsx) re-sent as `x-request-id` — the backend's own
  //   router already mints/propagates/logs `x-request-id` per request
  //   (see infrastructure/router.rs's TraceLayer span), so reusing that
  //   exact header name means every backend log line for this call already
  //   carries the SAME id Tauri and the browser console log against, with
  //   zero backend-side changes needed.
  fn forward_client_headers(
    mut req: reqwest::RequestBuilder,
    headers: &axum::http::HeaderMap,
  ) -> reqwest::RequestBuilder {
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
      if let Ok(v) = auth.to_str() {
        req = req.header("Authorization", v);
      }
    }
    if let Some(sid) = headers.get("x-session-id") {
      if let Ok(v) = sid.to_str() {
        req = req.header("x-request-id", v);
      }
    }
    req
  }

  async fn proxy_post(
    url: &str,
    body: serde_json::Value,
    headers: &axum::http::HeaderMap,
  ) -> axum::response::Response {
    let client = reqwest::Client::new();
    let req = forward_client_headers(client.post(url).json(&body), headers);
    match req.send().await {
      Ok(resp) => forward_backend_response(resp).await,
      Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    }
  }

  // Auth proxy routes — capture JWT from responses so GET /token can serve it
  async fn api_login(
    State(_s): State<LocalState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/login", get_backend_url()),
      body,
      &headers,
    )
    .await
  }
  async fn api_register(
    State(_s): State<LocalState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/register", get_backend_url()),
      body,
      &headers,
    )
    .await
  }
  async fn api_login_email(
    State(_s): State<LocalState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/login-email", get_backend_url()),
      body,
      &headers,
    )
    .await
  }
  async fn api_register_email(
    State(_s): State<LocalState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/register-email", get_backend_url()),
      body,
      &headers,
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
  async fn api_link_wallet(
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/link-wallet", get_backend_url()),
      body,
      &headers,
    )
    .await
  }
  async fn api_sync_profile(headers: axum::http::HeaderMap) -> impl IntoResponse {
    proxy_post(
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
    proxy_post(
      &format!("{}/api/auth/add-email", get_backend_url()),
      body,
      &headers,
    )
    .await
  }
  async fn api_me(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/me", get_backend_url());
    let req = forward_client_headers(client.get(&url), &headers);
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
    let req = forward_client_headers(client.patch(&url).json(&body), &headers);
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
    proxy_post(
      &format!("{}/api/auth/init-profile-tx", get_backend_url()),
      body,
      &headers,
    )
    .await
  }

  // POST /api/auth/broadcast-tx — broadcast a signed transaction (proxied)
  async fn api_broadcast_tx(
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    proxy_post(
      &format!("{}/api/auth/broadcast-tx", get_backend_url()),
      body,
      &headers,
    )
    .await
  }

  // GET /api/fresh-blockhash — lets wallet-ui refresh the blockhash on an
  // already-built unsigned tx immediately before the wallet extension's
  // signTransaction() call, instead of trusting whatever blockhash the
  // backend baked in when it originally built the tx (e.g.
  // /api/auth/init-profile-tx). That bake-time blockhash can go stale by the
  // time the user actually clicks through the extension's approval popup —
  // reproduced live as broadcast-tx 502ing with "Blockhash not found" even
  // though signing itself succeeded. Solana blockhashes are only valid for
  // ~60-90s; there's no bound on how long a real human takes to approve a
  // wallet popup, so the fix is to fetch as late as possible, not to make
  // the original build-time fetch happen faster. Proxies to the backend's
  // already-public, allow-listed `/api/rpc` (getLatestBlockhash is on that
  // list) rather than requiring wallet-ui to know the real backend URL
  // directly — same reasoning as every other `/api/*` route in this file.
  async fn api_fresh_blockhash() -> impl IntoResponse {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "getLatestBlockhash",
      // `finalized`, not `confirmed`: this blockhash is written into a
      // transaction that a wallet extension is about to be asked to sign, and
      // the extension identifies which cluster the transaction belongs to by
      // looking the blockhash up on its own selected cluster — at
      // `isBlockhashValid`'s default `finalized` commitment. A `confirmed`
      // blockhash is younger than the ~32 slot (~13s) finalization lag, so that
      // lookup returns false for a perfectly good devnet blockhash, and
      // Solflare refuses to sign with "Network mismatch: your current network
      // is set to devnet, but this transaction is for mainnet". Costs ~13s of
      // the ~60s validity window, which the callers' stale-blockhash retry
      // already covers. Mirrors the backend's `wallet_signable_blockhash`.
      "params": [{ "commitment": "finalized" }]
    });
    let resp = match client
      .post(format!("{}/api/rpc", get_backend_url()))
      .json(&body)
      .send()
      .await
    {
      Ok(r) => r,
      Err(e) => return (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
    };
    let value: serde_json::Value = match resp.json().await {
      Ok(v) => v,
      Err(e) => {
        return (
          StatusCode::BAD_GATEWAY,
          format!("bad blockhash response: {e}"),
        )
          .into_response()
      }
    };
    let blockhash = value["result"]["value"]["blockhash"].as_str();
    let last_valid_block_height = value["result"]["value"]["lastValidBlockHeight"].as_u64();
    match blockhash {
      Some(bh) => Json(serde_json::json!({
        "blockhash": bh,
        "lastValidBlockHeight": last_valid_block_height,
      }))
      .into_response(),
      None => (
        StatusCode::BAD_GATEWAY,
        format!("no blockhash in RPC response: {value}"),
      )
        .into_response(),
    }
  }

  // POST /api/set-backend-url — the game client posts its own resolved
  // vps_base() here (see open_wallet_browser in
  // src/multiplayer/solana/tauri_signer.rs) so this bridge proxies to
  // exactly the same backend the game client itself is talking to, instead
  // of independently re-deriving the same env vars and risking a
  // split-brain mismatch (see get_backend_url's doc comment).
  async fn api_set_backend_url(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    match body["url"].as_str() {
      Some(url) if !url.is_empty() => {
        set_backend_url_override(url.to_string());
        StatusCode::OK
      }
      _ => StatusCode::BAD_REQUEST,
    }
  }

  // GET /api/backend-url — lets wallet-ui log/display which backend this
  // session is actually proxying to, so a prod/local mismatch is visible in
  // the popup's own console instead of only surfacing as a mystery 502.
  async fn api_get_backend_url() -> impl IntoResponse {
    Json(serde_json::json!({
      "url": get_backend_url(),
      "explicit": backend_url_override_cell().lock().unwrap().is_some(),
    }))
  }

  // POST /api/ready — wallet-ui pings this once its React app has mounted
  // and is ready to render the login/sign UI. Closes the readiness gap
  // `open_in_browser` used to fly blind on: previously the only signal that
  // the popup was usable was the window-title poll (WINDOW_FOUND), which
  // only proves the OS window exists, not that React has actually taken
  // over the page — a slow bundle load left a window that LOOKED ready but
  // couldn't respond to anything yet. Body carries the `sid` this page read
  // from its own URL so the log line ties back to the exact OPEN_POPUP_START
  // that spawned it.
  async fn api_ready(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let sid = body["sid"].as_str().unwrap_or("-").to_string();
    mark_session_ready(&sid);
    StatusCode::OK
  }

  // POST /api/debug-log — temporary diagnostic passthrough so a specific
  // branch inside wallet-ui's JS (which only logs to that popup's own
  // browser console, awkward to get to in --app mode) shows up in this same
  // Tauri console instead. Tracking the repeated-login-loop bug where a
  // profile-less wallet somehow gets its popup hidden and the whole
  // WalletStep flow re-run instead of landing on ProfileStep.
  async fn api_debug_log(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let msg = body["msg"].as_str().unwrap_or("(no msg)");
    tracing::info!("[JS] {msg}");
    StatusCode::OK
  }

  // POST /api/open-profile-step — game client calls this when user tries to wager without
  // an on-chain profile. Sets a flag that the wallet-ui polls, and opens the popup.
  async fn api_open_profile_step(State(s): State<LocalState>) -> impl IntoResponse {
    s.needs_profile_step
      .store(true, std::sync::atomic::Ordering::Relaxed);
    let wallet_url = std::env::var("XFCHESS_WALLET_URL")
      .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
    let profile_url = format!("{wallet_url}?step=profile");
    tracing::info!("[HTTP] opening profile step: {profile_url}");
    tokio::task::spawn_blocking(move || {
      // false: wallet-ui's own splash-step poll loop (needs-profile-step)
      // already detects this and transitions client-side, so reusing a
      // popup that's already open is fine here — unlike the signing case.
      open_in_browser(&profile_url, false);
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
    headers: axum::http::HeaderMap,
  ) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let url = format!("{}/api/auth/check-wallet/{pubkey}", get_backend_url());
    let req = forward_client_headers(client.get(&url), &headers);
    match req.send().await {
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
  async fn serve_wallet_ui(State(s): State<LocalState>, uri: axum::http::Uri) -> impl IntoResponse {
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
      // Added for the Privy SDK, which requests a `.json` and may request a
      // `.wasm` for its crypto path. Served as application/octet-stream, a
      // browser refuses to execute the wasm and silently rejects the JSON —
      // producing a popup that fails with no useful error anywhere.
      Some("json") => "application/json",
      Some("wasm") => "application/wasm",
      Some("woff") => "font/woff",
      Some("ttf") => "font/ttf",
      Some("map") => "application/json",
      _ => "application/octet-stream",
    };

    // Cache policy. Vite fingerprints every asset (`index-<hash>.js`), so those
    // are immutable and safe to cache hard. `index.html` is NOT fingerprinted
    // and is the file that names which hashed bundle to load — and Vite *deletes*
    // the previous bundle on each rebuild. With no cache header at all, Chrome
    // heuristically cached index.html, so after a rebuild a still-open popup kept
    // asking for a bundle that no longer existed on disk: its only <script> 404'd
    // and the window rendered completely blank, with nothing in the UI to explain
    // it. Reproduced twice during development. HTML must always be revalidated.
    let is_html = mime.starts_with("text/html");
    let cache_control = if is_html {
      "no-store, must-revalidate"
    } else {
      "public, max-age=31536000, immutable"
    };

    match tokio::fs::read(&file_path).await {
      Ok(bytes) => axum::response::Response::builder()
        .header("Content-Type", mime)
        .header("Cache-Control", cache_control)
        .body(axum::body::Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
      Err(_) => {
        // Try index.html as SPA fallback
        match tokio::fs::read(dist.join("index.html")).await {
          Ok(bytes) => axum::response::Response::builder()
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Cache-Control", "no-store, must-revalidate")
            .body(axum::body::Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response()),
          Err(_) => (
            StatusCode::NOT_FOUND,
            format!("{prefix} dist not found. Build it first: cd tauri{prefix} && npm run build"),
          )
            .into_response(),
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
    let dev_path = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/wallet-ui/dist"));
    if dev_path.exists() {
      dev_path
    } else {
      std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("wallet-ui/dist")))
        .unwrap_or(dev_path)
    }
  };

  spawn_wallet_state_reaper(
    wallet_pubkey.clone(),
    wallet_username.clone(),
    wallet_provider.clone(),
    wallet_jwt.clone(),
    wallet_last_seen.clone(),
  );

  let state = LocalState {
    app,
    pending,
    notify,
    wallet_pubkey,
    wallet_username,
    wallet_provider,
    wallet_jwt,
    wallet_last_seen,
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
    .route("/api/fresh-blockhash", get(api_fresh_blockhash))
    .route("/api/game/launch", post(api_game_launch))
    .route("/api/open-profile-step", post(api_open_profile_step))
    .route("/api/needs-profile-step", get(api_needs_profile_step))
    .route("/api/set-backend-url", post(api_set_backend_url))
    .route("/api/backend-url", get(api_get_backend_url))
    .route("/api/ready", post(api_ready))
    .route("/api/debug-log", post(api_debug_log));

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

  match bind_http_port().await {
    Some((listener, port)) => {
      tracing::info!("[HTTP] Wallet bridge listening on http://localhost:{port}");
      if let Err(e) = axum::serve(listener, router).await {
        tracing::error!("[HTTP] Wallet bridge error: {e}");
      }
    }
    None => tracing::error!(
      "[HTTP] Failed to bind wallet bridge on :{}-{}: all candidate ports in use",
      nominal_http_port(),
      nominal_http_port().saturating_add(10)
    ),
  }
}

// ---------------------------------------------------------------------------
// Session lifecycle correlation — a fresh `sid` is minted every time the
// popup is (re)opened for a login/sign attempt, threaded through the popup
// URL, echoed back by wallet-ui as an `X-Session-Id` header on every fetch,
// and forwarded to the backend as `x-request-id` (see `forward_session_id`)
// so the *same* ID appears in Tauri's log, the browser console, and the
// backend's request-scoped tracing spans for one end-to-end attempt — the
// only way to tell, after the fact, which Chrome-spawn/window-discovery/
// wallet-relay/backend-call log lines all belong to the same click.
// ---------------------------------------------------------------------------

struct PopupSession {
  id: String,
  opened_at: std::time::Instant,
  ready_logged: bool,
}

fn current_session_cell() -> &'static std::sync::Mutex<Option<PopupSession>> {
  static CELL: std::sync::OnceLock<std::sync::Mutex<Option<PopupSession>>> =
    std::sync::OnceLock::new();
  CELL.get_or_init(|| std::sync::Mutex::new(None))
}

fn new_session_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

/// Start tracking a new popup session, logging `OPEN_POPUP_START`, and
/// return its ID for embedding into the popup URL.
fn begin_session() -> String {
  let id = new_session_id();
  tracing::info!(sid = %id, event = "OPEN_POPUP_START", "[Lifecycle] OPEN_POPUP_START");
  *current_session_cell().lock().unwrap() = Some(PopupSession {
    id: id.clone(),
    opened_at: std::time::Instant::now(),
    ready_logged: false,
  });
  id
}

/// Log `WINDOW_FOUND` (or the timeout case) against whichever session is
/// current, if any — `resize_wallet_popup_window`/the resize watcher don't
/// otherwise know the sid, since they're matched purely by OS window title.
fn log_window_event(event: &str) {
  let guard = current_session_cell().lock().unwrap();
  if let Some(s) = guard.as_ref() {
    tracing::info!(
      sid = %s.id, event = %event, elapsed_ms = s.opened_at.elapsed().as_millis() as u64,
      "[Lifecycle] {event}"
    );
  }
}

/// Called from `POST /api/ready` once wallet-ui's React app has mounted.
/// `sid` comes from the page itself (read from its own URL) rather than
/// trusting "whichever session is current" — a stale/reused popup page
/// pinging this after a newer session has already started would otherwise
/// misattribute REACT_READY to the wrong attempt.
fn mark_session_ready(sid: &str) {
  let mut guard = current_session_cell().lock().unwrap();
  if let Some(s) = guard.as_mut() {
    if s.id == sid && !s.ready_logged {
      s.ready_logged = true;
      tracing::info!(
        sid = %sid, event = "REACT_READY", elapsed_ms = s.opened_at.elapsed().as_millis() as u64,
        "[Lifecycle] REACT_READY"
      );
      return;
    }
  }
  drop(guard);
  if !sid.is_empty() && sid != "-" {
    tracing::debug!(sid = %sid, "[Lifecycle] REACT_READY for a session that is no longer current (stale popup page)");
  }
}

/// Open the wallet UI in the user's real Chrome browser so Phantom/Solflare
/// extensions are available. WebView2 inside Tauri cannot load extensions.
fn open_wallet_popup(_app: &tauri::AppHandle) {
  open_wallet_popup_with_step(None, false);
}

/// Open the wallet UI to approve a pending transaction. Passing `?step=sign`
/// tells wallet-ui (see hasExistingSession in App.tsx) to skip straight past
/// the login/profile walkthrough when a session is already on disk — a plain
/// `open_wallet_popup()` reopens the base URL, which always restarts at
/// consent/entry, so a signing request that arrives after the user already
/// logged in used to show a fresh "log in again" screen instead of the sign
/// prompt, and the pending tx would silently time out 60s later.
fn open_wallet_popup_for_signing(_app: &tauri::AppHandle) {
  // force_fresh=true: reusing whatever the popup already had loaded (e.g.
  // still sitting on the splash screen from an earlier, unrelated open) is
  // exactly the bug this function's own doc comment above describes fixing
  // once already — the reuse-by-title-match optimization in open_in_browser
  // brings that stale window to the foreground WITHOUT navigating it, so a
  // signing request silently never reaches the sign screen, and the pending
  // tx (still handled correctly by TransactionSigner's own SSE-driven state
  // regardless of `step`) times out with nothing on screen prompting it.
  // A fresh popup guarantees today's ?step=sign URL actually loads.
  open_wallet_popup_with_step(Some("sign"), true);
}

fn open_wallet_popup_with_step(step: Option<&str>, force_fresh: bool) {
  let sid = begin_session();
  let wallet_url = std::env::var("XFCHESS_WALLET_URL")
    .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
  let url = match step {
    Some(s) => format!("{wallet_url}?step={s}&sid={sid}"),
    None => format!("{wallet_url}?sid={sid}"),
  };
  tracing::info!(sid = %sid, "[WalletPopup] opening in system browser: {url}");
  if force_fresh {
    kill_wallet_popup();
  }
  open_in_browser(&url, force_fresh);
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

fn open_in_browser(url: &str, force_fresh: bool) {
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
    // Skipped when force_fresh: reusing it means whatever page it already
    // had loaded stays loaded — no navigation happens — which silently
    // stranded signing requests behind a stale splash/login screen (see
    // open_wallet_popup_for_signing's doc comment). The caller already
    // killed any existing popup before reaching here in that case, so this
    // check would find nothing anyway; skipping it outright avoids a race
    // against how quickly that close actually takes effect.
    if !force_fresh {
      if show_and_foreground_wallet_popup() {
        tracing::info!("[WalletPopup] reused existing popup window for {url_ts}");
        resize_wallet_popup_window(WALLET_POPUP_WIDTH, WALLET_POPUP_HEIGHT);
        return;
      }
      // No window matched `XFChess #{http_port()}` — either none exists yet,
      // or a prior one was actually closed (not just hidden) between calls.
      // Falling through spawns a brand-new browser process at `url_ts`,
      // which is a FULL fresh page load: React remounts from scratch, so
      // whatever step the popup was previously on (e.g. mid ProfileStep) is
      // lost and the whole WalletStep login flow runs again. Bumped to
      // `info!` (was `debug!`, invisible at the default log level) because
      // this exact silent fallthrough is the leading suspect for the
      // "repeated login loop" bug — a caller that assumed reuse (e.g.
      // `open_profile_step`'s `force_fresh: false`) gets a full respawn
      // instead every time this fires.
      tracing::info!(
        "[WalletPopup] no existing popup window found for {url_ts} — spawning a fresh one \
         (this will restart the login flow if one was already in progress)"
      );
    }

    // Windows console apps like `reg.exe` allocate their own visible console
    // window when spawned from a windows-subsystem (console-less) parent —
    // CREATE_NO_WINDOW suppresses that so these lookups stay invisible.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn get_chromium_default_browser() -> Option<String> {
      let output = std::process::Command::new("reg")
        .args([
          "query",
          r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
          "/v",
          "ProgId",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
      let stdout = String::from_utf8_lossy(&output.stdout);
      let prog_id = stdout
        .lines()
        .find(|l| l.contains("ProgId"))?
        .split_whitespace()
        .last()?;

      let hkcr = format!(r"HKCR\{}\shell\open\command", prog_id);
      let output = std::process::Command::new("reg")
        .args(["query", &hkcr, "/ve"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
      let stdout = String::from_utf8_lossy(&output.stdout);

      let path_str = stdout.lines().find(|l| l.contains("REG_SZ"))?;
      let idx = path_str.find("REG_SZ")?;
      let cmd = path_str[idx + 6..].trim();
      let path = if cmd.starts_with('"') {
        let end_quote = cmd[1..].find('"')?;
        cmd[1..end_quote + 1].to_string()
      } else {
        let path_part = cmd
          .split(" --")
          .next()
          .unwrap_or(cmd)
          .split(" %")
          .next()
          .unwrap_or(cmd);
        path_part.trim().to_string()
      };
      let lower = path.to_lowercase();
      if lower.contains("chrome.exe")
        || lower.contains("msedge.exe")
        || lower.contains("brave.exe")
        || lower.contains("vivaldi.exe")
        || lower.contains("opera.exe")
      {
        return Some(path);
      }
      None
    }

    if let Some(chromium_browser) = get_chromium_default_browser() {
      if std::path::Path::new(&chromium_browser).exists() {
        let app_flag = format!("--app={}", url_ts);
        match Command::new(&chromium_browser)
          .args([
            &app_flag,
            &format!("--window-size={WALLET_POPUP_WIDTH},{WALLET_POPUP_HEIGHT}"),
          ])
          .spawn()
        {
          Ok(child) => {
            let pid = child.id();
            *wallet_popup_pid_cell().lock().unwrap() = Some(pid);
            spawn_wallet_popup_resize_watcher();
            return;
          }
          Err(e) => tracing::warn!("[WalletPopup] failed to spawn {chromium_browser}: {e}"),
        }
      }
    }

    // Fall back to default browser via open::that() if not Chromium or spawn failed
    tracing::info!("[WalletPopup] Opening in default browser (not chromium --app)");
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
// ── Admin SSH tunnel (PRODUCTION mode) ───────────────────────────────────────
//
// Owned here in Rust rather than in the panel's JavaScript, because the
// JS-side version had no way to guarantee the child process died. Closing the
// admin window or rebuilding the UI dropped the JS reference but left the real
// `ssh.exe` running, and those orphans then squatted port 8091 so every
// *subsequent* tunnel silently failed to bind — presenting as "tunnel down"
// while SSH auth itself was working fine. Tying the child to app state plus
// window-close/app-exit hooks makes that failure structurally impossible.
//
// See docs/plans/tournament-admin-connection-rearchitecture.md §3 Phase 2.
#[cfg(feature = "tournament-admin")]
#[derive(Default)]
struct AdminTunnel(Arc<Mutex<Option<tauri_plugin_shell::process::CommandChild>>>);

/// Probes `http://127.0.0.1:{port}/health` from Rust (no browser, so no CORS).
#[cfg(feature = "tournament-admin")]
async fn admin_health_ok(port: u16) -> bool {
  let url = format!("http://127.0.0.1:{port}/health");
  match reqwest::Client::new()
    .get(&url)
    .timeout(std::time::Duration::from_secs(3))
    .send()
    .await
  {
    Ok(r) => r.status().is_success(),
    Err(_) => false,
  }
}

/// Brings up the PRODUCTION SSH tunnel and returns once the backend actually
/// answers through it. Idempotent: if a healthy tunnel is already listening
/// (ours or one you opened by hand in a terminal) it is reused rather than
/// duplicated.
#[cfg(feature = "tournament-admin")]
#[tauri::command]
async fn ensure_admin_tunnel(
  app: tauri::AppHandle,
  key_path: String,
  ssh_user: String,
  ssh_host: String,
  local_port: u16,
  remote_host: String,
  remote_port: u16,
) -> Result<String, String> {
  use tauri_plugin_shell::ShellExt;

  // Already up and healthy? Reuse it.
  if admin_health_ok(local_port).await {
    return Ok("reused".into());
  }

  // Drop any child we previously spawned before binding the port again.
  kill_admin_tunnel_inner(&app);

  // Port occupied but NOT answering /health => a stale squatter (very likely an
  // orphan from an older build). Say so precisely instead of blaming the tunnel.
  if std::net::TcpStream::connect(("127.0.0.1", local_port)).is_ok() {
    return Err(format!(
      "Port {local_port} is already in use by another process, but it is not \
       answering /health — it's most likely a stale ssh.exe from an earlier \
       session. Close it (Task Manager, or `taskkill /F /IM ssh.exe`) and try again."
    ));
  }

  let forward = format!("{local_port}:{remote_host}:{remote_port}");
  let target = format!("{ssh_user}@{ssh_host}");
  let child = app
    .shell()
    .command("ssh")
    .args([
      "-i",
      &key_path,
      "-o",
      "BatchMode=yes",
      "-o",
      "ExitOnForwardFailure=yes",
      "-o",
      "ServerAliveInterval=30",
      "-o",
      "StrictHostKeyChecking=accept-new",
      "-N",
      "-L",
      &forward,
      &target,
    ])
    .spawn()
    .map(|(_rx, child)| child)
    .map_err(|e| format!("could not start ssh: {e}"))?;

  if let Ok(mut slot) = app.state::<AdminTunnel>().0.lock() {
    *slot = Some(child);
  }

  // Poll rather than guessing a fixed delay: a cold handshake to a real VPS
  // (TCP + host key + auth + forward setup) is routinely slower than the 8s
  // the old JS timeout allowed, which is why it reported failure moments
  // before the tunnel actually came up.
  for _ in 0..30 {
    if admin_health_ok(local_port).await {
      return Ok("connected".into());
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
  }

  kill_admin_tunnel_inner(&app);
  Err(format!(
    "SSH connected but the backend never answered /health on port {local_port} \
     within 15s. Check that the '{ssh_user}' user exists on {ssh_host}, that \
     your key is authorized, and that the backend is running."
  ))
}

#[cfg(feature = "tournament-admin")]
fn kill_admin_tunnel_inner(app: &tauri::AppHandle) {
  if let Some(state) = app.try_state::<AdminTunnel>() {
    if let Ok(mut slot) = state.0.lock() {
      if let Some(child) = slot.take() {
        let _ = child.kill();
      }
    }
  }
}

#[cfg(feature = "tournament-admin")]
#[tauri::command]
fn kill_admin_tunnel(app: tauri::AppHandle) {
  kill_admin_tunnel_inner(&app);
}

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

/// Target size for the wallet popup window — a compact sign-in card, not a
/// full browser window. Must match the `--window-size` flag passed at spawn
/// in `open_in_browser`.
const WALLET_POPUP_WIDTH: i32 = 460;
const WALLET_POPUP_HEIGHT: i32 = 720;

/// Force the popup window to `WALLET_POPUP_WIDTH`x`WALLET_POPUP_HEIGHT`,
/// keeping its current top-left position. `--window-size` on spawn is not
/// enough on its own: this popup deliberately runs in the user's real Chrome
/// profile (see `kill_wallet_popup`'s doc comment), and Chrome restores the
/// *previous* app window's remembered bounds from that shared profile after
/// creation, silently overriding the CLI flag — which is why a user dragging
/// the window bigger once makes every future popup open oversized, forever,
/// no matter what the spawn flag says. Same title/owning-process match as
/// `kill_wallet_popup`.
#[cfg(windows)]
fn resize_wallet_popup_window(width: i32, height: i32) -> bool {
  use ::windows::core::BOOL;
  use ::windows::Win32::Foundation::{HWND, LPARAM};
  use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
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
    let _ = SetWindowPos(
      ctx.1,
      None,
      0,
      0,
      width,
      height,
      SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
  }
  true
}

#[cfg(not(windows))]
fn resize_wallet_popup_window(_width: i32, _height: i32) -> bool {
  false
}

/// Enforces the popup's fixed size shortly after a fresh spawn. Two races to
/// cover, not one:
///  - The window doesn't exist the instant `Command::spawn` returns — Chrome
///    forwards the `--app=` request over its single-instance IPC and the
///    actual top-level window shows up some tens to hundreds of ms later.
///  - The match is by *title* (`resize_wallet_popup_window`'s doc comment
///    explains why PID matching isn't reliable here), and the title is only
///    stamped once `wallet-ui`'s JS bundle finishes loading and evaluates
///    `document.title = ...` (see App.tsx) — under load (e.g. two full game
///    instances running at once, as with `just dev2`) that can take longer
///    than a short poll window, so a watcher that gives up too early leaves
///    the window at whichever size Chrome's shared profile happened to
///    restore it to.
/// Keeps reasserting for the whole window (not just until the first hit)
/// since Chrome has also been observed to apply its own remembered bounds
/// for that shared profile *after* creation, silently undoing a one-shot fix.
/// Total time to keep polling for the popup window before giving up
/// (`POLL_INTERVAL_MS` * `POLL_ATTEMPTS`). Was 8s (40*200ms), which is
/// comfortable for a warm Chrome process but not for a genuinely cold start
/// (first launch after reboot, machine under load, antivirus scanning the
/// new process) — that's exactly the `gave up waiting for popup window to
/// enforce its size` case, where the window shows up eventually just not
/// within the old budget. 30s covers cold-start without leaving a runaway
/// watcher: this task always exits once the loop ends regardless.
const POPUP_WINDOW_POLL_INTERVAL_MS: u64 = 200;
const POPUP_WINDOW_POLL_ATTEMPTS: u32 = 150;

fn spawn_wallet_popup_resize_watcher() {
  tauri::async_runtime::spawn(async move {
    let mut found_once = false;
    for _ in 0..POPUP_WINDOW_POLL_ATTEMPTS {
      tokio::time::sleep(std::time::Duration::from_millis(
        POPUP_WINDOW_POLL_INTERVAL_MS,
      ))
      .await;
      if resize_wallet_popup_window(WALLET_POPUP_WIDTH, WALLET_POPUP_HEIGHT) {
        if !found_once {
          log_window_event("WINDOW_FOUND");
        }
        found_once = true;
      }
    }
    if !found_once {
      log_window_event("WINDOW_NOT_FOUND_TIMEOUT");
      tracing::warn!("[WalletPopup] gave up waiting for popup window to enforce its size");
    }
  });
}

/// How long the bridge can go without a `/status` poll before it forgets the
/// connected wallet. The game client polls every 5s while running (see
/// `poll_wallet_bridge`) — this is a generous multiple of that, not a tight
/// timeout, so a brief hitch never disconnects an active session.
const WALLET_STATE_IDLE_CLEAR_SECS: u64 = 30;

/// Background task, run for the app's lifetime: this bridge process outlives
/// the game window on purpose (closing the game doesn't kill it, so a later
/// launch can reconnect fast) — but with no expiry, that meant the connected
/// wallet (pubkey + username + JWT) stayed cached here indefinitely, across
/// every subsequent game launch, until someone manually killed the process.
/// A player who closed the game, came back hours/days later, and connected a
/// *different* wallet on the website would still see the old wallet's
/// username in the game client, because it was never told anything changed —
/// the two are entirely separate connections (see `post_wallet`'s doc
/// comment). Clearing the cache once nothing has polled `/status` for
/// `WALLET_STATE_IDLE_CLEAR_SECS` ties the cached identity to "a game is
/// actually running and asking," which is the right lifetime for it, without
/// needing to track the game process's PID directly.
fn spawn_wallet_state_reaper(
  wallet_pubkey: WalletPubkey,
  wallet_username: WalletUsername,
  wallet_provider: WalletProvider,
  wallet_jwt: WalletJwt,
  wallet_last_seen: WalletLastSeen,
) {
  tauri::async_runtime::spawn(async move {
    loop {
      tokio::time::sleep(std::time::Duration::from_secs(10)).await;
      let stale = {
        let last_seen = wallet_last_seen.0.lock().unwrap();
        match *last_seen {
          Some(at) => at.elapsed().as_secs() >= WALLET_STATE_IDLE_CLEAR_SECS,
          // Never polled at all — nothing to clear yet, this isn't staleness.
          None => false,
        }
      };
      if stale && wallet_pubkey.0.lock().unwrap().is_some() {
        tracing::info!(
          "[WalletState] no /status poll for {WALLET_STATE_IDLE_CLEAR_SECS}s — clearing cached wallet"
        );
        *wallet_pubkey.0.lock().unwrap() = None;
        *wallet_username.0.lock().unwrap() = None;
        *wallet_provider.0.lock().unwrap() = None;
        *wallet_jwt.0.lock().unwrap() = None;
      }
    }
  });
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
    //
    // XFCHESS_ADMIN_DEV_URL overrides this with a Vite dev server (see
    // `just admin-dev`), giving hot-module reload on UI edits instead of a
    // full rebuild-and-relaunch cycle. Loopback-only by assertion below: a
    // stray env var must never be able to point this window at a remote
    // origin, since it holds admin credentials and shell/tunnel permissions.
    let admin_url = match std::env::var("XFCHESS_ADMIN_DEV_URL") {
      Ok(dev) if !dev.trim().is_empty() => {
        let dev = dev.trim().to_string();
        let is_loopback =
          dev.starts_with("http://localhost:") || dev.starts_with("http://127.0.0.1:");
        if is_loopback {
          tracing::warn!("[TournamentAdmin] DEV MODE — loading from {dev} (hot reload)");
          dev
        } else {
          tracing::error!(
            "[TournamentAdmin] XFCHESS_ADMIN_DEV_URL={dev} is not loopback — ignoring"
          );
          format!("http://localhost:{}/tournament-admin/", http_port())
        }
      }
      _ => format!("http://localhost:{}/tournament-admin/", http_port()),
    };
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
          // Closing the admin window must take the SSH tunnel with it —
          // otherwise the orphaned ssh.exe keeps port 8091 bound and the next
          // login silently fails to establish a forward.
          let tunnel_app = app.clone();
          win.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
              kill_admin_tunnel_inner(&tunnel_app);
            }
          });
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
    // Lets the tournament-admin webview issue backend calls from Rust rather
    // than from the browser context — see the dependency comment in
    // Cargo.toml. Scoped to loopback admin ports by capabilities/admin-http.json.
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    .setup(|app| {
      // Always start disconnected — user must connect a wallet each session.
      let wallet_pubkey = WalletPubkey::default();
      let wallet_username = WalletUsername::default();
      let wallet_provider = WalletProvider::default();
      let wallet_jwt = WalletJwt::default();
      let wallet_last_seen = WalletLastSeen::default();
      let pending_tx: PendingTx = Arc::new(Mutex::new(None));
      let (pending_notify, _): (PendingTxNotify, _) = tokio::sync::watch::channel(());
      let auth_state = services::auth::AuthState::new();

      // Register shared state with Tauri app
      app.manage(wallet_pubkey.clone());
      app.manage(wallet_username.clone());
      app.manage(wallet_provider.clone());
      app.manage(wallet_jwt.clone());
      app.manage(wallet_last_seen.clone());
      app.manage(pending_tx.clone());
      app.manage(auth_state);
      #[cfg(feature = "tournament-admin")]
      app.manage(AdminTunnel::default());

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
        let wp = wallet_provider.clone();
        let wj = wallet_jwt.clone();
        let wls = wallet_last_seen.clone();
        tauri::async_runtime::spawn(http_server(h, p, n, w, wu, wp, wj, wls));
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
      #[cfg(feature = "tournament-admin")]
      ensure_admin_tunnel,
      #[cfg(feature = "tournament-admin")]
      kill_admin_tunnel,
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
        // Same reasoning for the admin SSH tunnel: an orphaned ssh.exe holds
        // port 8091 and breaks every later tunnel attempt.
        #[cfg(feature = "tournament-admin")]
        kill_admin_tunnel_inner(_app_handle);
      }
    });
}
