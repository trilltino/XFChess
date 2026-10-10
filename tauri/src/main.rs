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


#[allow(dead_code)]
#[derive(Default, Clone)]
struct WalletPubkey(Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
struct WalletUsername(Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
struct WalletProvider(Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
struct WalletJwt(Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
struct WalletLastSeen(Arc<Mutex<Option<std::time::Instant>>>);

struct PendingRequest {
  id: String,
  tx: Vec<u8>,
  label: String,
  response: oneshot::Sender<Result<Vec<u8>, String>>,
}

type PendingTxInner = Option<PendingRequest>;
type PendingTx = Arc<Mutex<PendingTxInner>>;

type PendingTxNotify = tokio::sync::watch::Sender<()>;

const SIGN_TIMEOUT_SECS: u64 = 60;

static ACTUAL_HTTP_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

fn nominal_http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

fn http_port() -> u16 {
  ACTUAL_HTTP_PORT
    .get()
    .copied()
    .unwrap_or_else(nominal_http_port)
}

fn http_bridge_port_file() -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-http-{}.port", nominal_http_port()))
}

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

fn wallet_bridge_port_file(base_port: u16) -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-bridge-{base_port}.port"))
}

static BACKEND_URL_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
  std::sync::OnceLock::new();

fn backend_url_override_cell() -> &'static std::sync::Mutex<Option<String>> {
  BACKEND_URL_OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

fn get_backend_url() -> String {
  if let Some(url) = backend_url_override_cell().lock().unwrap().clone() {
    return url;
  }
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "https://xfchess.com".to_string())
}

fn set_backend_url_override(url: String) {
  let mut cell = backend_url_override_cell().lock().unwrap();
  let changed = cell.as_deref() != Some(url.as_str());
  if changed {
    tracing::info!("[Backend] target set by game client: {url}");
  }
  *cell = Some(url);
}

fn instance_cache_dir() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("xfchess")
    .join(format!("port-{}", http_port()))
}

fn consent_path() -> PathBuf {
  instance_cache_dir().join("consent.json")
}

// Loopback HTTP bridge for wallet signing state and backend API proxying.

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
    let tx_b64 = lock.as_ref().map(|request| B64.encode(&request.tx));
    let label = lock.as_ref().map(|request| request.label.clone());
    let request_id = lock.as_ref().map(|request| request.id.clone());
    serde_json::json!({ "tx": tx_b64, "label": label, "request_id": request_id })
  }

  // Plain-fetch fallback for SSE: return {"tx":"<b64>","label":"<str>"} or {"tx":null}.
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

  // SSE emits current pending state on connect and whenever it changes.
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

  // POST /resolved — wallet-ui posts {"request_id":"...","signed":"<b64>"}
  async fn post_resolved(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let request_id = body["request_id"].as_str().unwrap_or("");
    let signed_b64 = body["signed"].as_str().unwrap_or("").to_string();
    let mut lock = s.pending.lock().unwrap();
    let Some(request) = lock.as_ref() else {
      tracing::warn!(
        request_id,
        event = "STALE_RESOLVED",
        "no active signing request"
      );
      return StatusCode::CONFLICT;
    };
    if request.id != request_id {
      tracing::warn!(request_id, active_id = %request.id, event = "STALE_RESOLVED", "request id mismatch");
      return StatusCode::CONFLICT;
    }
    if let Some(request) = lock.take() {
      let sender = request.response;
      tracing::info!(
        request_id,
        event = "SIGN_RESOLVED",
        cancelled = signed_b64.is_empty()
      );
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

  // POST /cancel — wallet-ui posts {"request_id":"...","reason":"..."}
  async fn post_cancel(
    State(s): State<LocalState>,
    Json(body): Json<serde_json::Value>,
  ) -> impl IntoResponse {
    let request_id = body["request_id"].as_str().unwrap_or("");
    let reason = body["reason"].as_str().unwrap_or("Cancelled by wallet UI");
    let mut lock = s.pending.lock().unwrap();
    let Some(request) = lock.as_ref() else {
      return StatusCode::CONFLICT;
    };
    if request.id != request_id {
      tracing::warn!(request_id, active_id = %request.id, event = "STALE_CANCEL", "request id mismatch");
      return StatusCode::CONFLICT;
    }
    if let Some(request) = lock.take() {
      let _ = request.response.send(Err(reason.to_string()));
    }
    drop(lock);
    let _ = s.notify.send(());
    tracing::info!(request_id, event = "SIGN_CANCELLED", reason);
    StatusCode::OK
  }

  // An empty username explicitly clears the cache; an absent field leaves it
  // unchanged. Keep these distinct when wallets share a browser profile.
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

  // Clear cached wallet identity on logout so a later login cannot inherit the prior session.
  async fn post_wallet_disconnect(State(s): State<LocalState>) -> impl IntoResponse {
    *s.wallet_pubkey.0.lock().unwrap() = None;
    *s.wallet_username.0.lock().unwrap() = None;
    *s.wallet_provider.0.lock().unwrap() = None;
    *s.wallet_jwt.0.lock().unwrap() = None;
    tracing::info!("[HTTP] Wallet disconnected and bridge auth state cleared");
    StatusCode::OK
  }

  // Hide resolved signing popups for reuse; the idle reaper later closes them.
  async fn post_hide(_state: State<LocalState>) -> impl IntoResponse {
    hide_wallet_popup();
    StatusCode::OK
  }

  // Client /status polling also signals liveness to spawn_wallet_state_reaper.
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

  // Surface invalid backend response bodies clearly instead of a JSON decoding error.
  fn backend_unreachable_msg(e: reqwest::Error) -> String {
    tracing::warn!("[HTTP] backend request failed: {e}");
    "Could not reach the backend service. Please check it's running and try again.".to_string()
  }
  fn backend_bad_response_msg(e: reqwest::Error) -> String {
    tracing::warn!("[HTTP] backend returned a non-JSON response: {e}");
    "The backend returned an unexpected response. Please try again in a moment.".to_string()
  }

  // Read the body once. Preserve its status and return JSON or plain text
  // without discarding backend diagnostics.
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

  // Forward Authorization for wallet authentication and X-Session-Id as
  // x-request-id for browser/bridge/backend log correlation.
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
        // Mirror a confirmed rename into bridge state for the game status poller.
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

  // Refresh unsigned transaction blockhashes immediately before wallet signing
  // through the backend RPC proxy; build-time hashes may have expired.
  async fn api_fresh_blockhash() -> impl IntoResponse {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "getLatestBlockhash",
      // Use finalized blockhashes to match extension wallets' cluster-validity lookups.
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

  // Use the backend URL resolved by the game so bridge and client share one endpoint.
  async fn api_set_backend_url(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    match body["url"].as_str() {
      Some(url) if !url.is_empty() => {
        set_backend_url_override(url.to_string());
        StatusCode::OK
      }
      _ => StatusCode::BAD_REQUEST,
    }
  }

  async fn api_get_backend_url() -> impl IntoResponse {
    Json(serde_json::json!({
      "url": get_backend_url(),
      "explicit": backend_url_override_cell().lock().unwrap().is_some(),
    }))
  }

  // Record React readiness separately from OS window discovery, correlated by sid.
  async fn api_ready(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let sid = body["sid"].as_str().unwrap_or("-").to_string();
    mark_session_ready(&sid);
    StatusCode::OK
  }

  // Forward wallet UI diagnostics to the bridge log for popup debugging.
  async fn api_debug_log(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let msg = body["msg"].as_str().unwrap_or("(no msg)");
    tracing::info!("[JS] {msg}");
    StatusCode::OK
  }

  // Request profile setup, flag the wallet UI, and open its popup.
  async fn api_open_profile_step(State(s): State<LocalState>) -> impl IntoResponse {
    s.needs_profile_step
      .store(true, std::sync::atomic::Ordering::Relaxed);
    let wallet_url = std::env::var("XFCHESS_WALLET_URL")
      .unwrap_or_else(|_| format!("http://localhost:{}/wallet-ui/", http_port()));
    let profile_url = format!("{wallet_url}?step=profile");
    tracing::info!("[HTTP] opening profile step: {profile_url}");
    tokio::task::spawn_blocking(move || {
      // The popup polls needs-profile-step and can transition without being recreated.
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

  // Serve built admin assets only with tournament-admin enabled; consumer
  // releases exclude this route, window, and IPC surface.
  #[cfg(feature = "tournament-admin")]
  async fn serve_tournament_admin(
    State(s): State<LocalState>,
    uri: axum::http::Uri,
  ) -> impl IntoResponse {
    serve_dist_file(&s.dist_path, "/tournament-admin", uri.path()).await
  }

  // Always serve wallet UI assets from the loopback bridge for shipped signing flows.
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
      // Privy requires JSON and WASM MIME types; octet-stream prevents browser loading.
      Some("json") => "application/json",
      Some("wasm") => "application/wasm",
      Some("woff") => "font/woff",
      Some("ttf") => "font/ttf",
      Some("map") => "application/json",
      _ => "application/octet-stream",
    };

    // Revalidate index.html so rebuilt hash filenames are discovered. Fingerprinted
    // assets may be cached immutably.
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

  // Reflect only local/webview origins to prevent arbitrary websites reading bridge tokens.
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
    .route("/cancel", post(post_cancel))
    .route("/wallet", post(post_wallet))
    .route("/wallet/disconnect", post(post_wallet_disconnect))
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

  // Serve the admin UI only when compiled with tournament-admin.
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

// Correlate each popup attempt with sid across its URL, X-Session-Id, and
// backend x-request-id.

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

fn log_window_event(event: &str) {
  let guard = current_session_cell().lock().unwrap();
  if let Some(s) = guard.as_ref() {
    tracing::info!(
      sid = %s.id, event = %event, elapsed_ms = s.opened_at.elapsed().as_millis() as u64,
      "[Lifecycle] {event}"
    );
  }
}

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

fn open_wallet_popup(_app: &tauri::AppHandle) {
  open_wallet_popup_with_step(None, false);
}

fn open_wallet_popup_for_signing(_app: &tauri::AppHandle) {
  // Force a fresh sign URL; reusing a popup does not navigate away from stale steps.
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
    // Reuse matching popup windows unless force_fresh requires navigation to a new step.
    if !force_fresh {
      if show_and_foreground_wallet_popup() {
        tracing::info!("[WalletPopup] reused existing popup window for {url_ts}");
        resize_wallet_popup_window(WALLET_POPUP_WIDTH, WALLET_POPUP_HEIGHT);
        return;
      }
      // No title match means a fresh browser page; React state from the old popup is lost.
      tracing::info!(
        "[WalletPopup] no existing popup window found for {url_ts} — spawning a fresh one \
         (this will restart the login flow if one was already in progress)"
      );
    }

    // CREATE_NO_WINDOW prevents spawned Windows console tools from showing a console.
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

// Tie SSH children to Rust app state and window/app shutdown so closed panels
// cannot leave orphan tunnels holding the port.
#[cfg(feature = "tournament-admin")]
#[derive(Default)]
struct AdminTunnel(Arc<Mutex<Option<tauri_plugin_shell::process::CommandChild>>>);

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

  kill_admin_tunnel_inner(&app);

  // An occupied port without /health indicates a stale process, not a tunnel failure.
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

  // Poll readiness: cold SSH handshakes may exceed a fixed connection delay.
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

fn wallet_popup_hidden_at_cell() -> &'static std::sync::Mutex<Option<std::time::Instant>> {
  static CELL: std::sync::OnceLock<std::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::OnceLock::new();
  CELL.get_or_init(|| std::sync::Mutex::new(None))
}

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

const WALLET_POPUP_WIDTH: i32 = 460;
const WALLET_POPUP_HEIGHT: i32 = 720;

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

const WALLET_STATE_IDLE_CLEAR_SECS: u64 = 30;

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

const HIDDEN_POPUP_IDLE_KILL_SECS: u64 = 15 * 60;

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

// Consumer builds omit tournament-admin and cannot create this window.
fn apply_xfchess_window_icon(window: &tauri::WebviewWindow, app: &tauri::AppHandle) {
  let icon = app
    .default_window_icon()
    .cloned()
    .or_else(|| tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")).ok());
  if let Some(icon) = icon {
    if let Err(e) = window.set_icon(icon) {
      tracing::warn!("[WindowIcon] failed to set icon for {}: {e}", window.label());
    }
  } else {
    tracing::warn!("[WindowIcon] no XFChess window icon was available");
  }
}

#[cfg(feature = "tournament-admin")]
fn open_tournament_admin(app: &tauri::AppHandle) {
  // Window creation MUST run on the main thread in Tauri v2.
  let app2 = app.clone();
  let _ = app.run_on_main_thread(move || {
    let app = app2;
    // Serve the admin UI locally. XFCHESS_ADMIN_DEV_URL may override with a
    // loopback dev server only because this window has privileged capabilities.
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
          apply_xfchess_window_icon(&win, &app);
          // Close the SSH tunnel with the admin window so its forwarded port is released.
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


fn main() {
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

      if let Some(win) = app.get_webview_window("main") {
        apply_xfchess_window_icon(&win, app.handle());
      }

      // The wallet UI receives pending transactions over SSE and posts resolutions;
      // /token exposes its authenticated JWT to the game.
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

      #[cfg(feature = "tournament-admin")]
      {
        let _ = TournamentAdminWindow::new(app.handle());
      }

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

      // TCP accepts OPEN or [u32 LE label length][UTF-8 label][u32 LE tx length][tx].
      // Return [u32 LE length][signed bytes], or 0xFFFFFFFF for rejection.
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
            // Bind base-11 through base-2 in the same order as the client fallback scan.
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

                  // Answer wallet queries with the recorded pubkey or zero-length disconnected
                  // response; do not parse them as signing requests.
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
                  let request_id = uuid::Uuid::new_v4().to_string();
                  let request_busy = {
                    let mut guard = pending2.lock().unwrap();
                    if guard.is_some() {
                      true
                    } else {
                      *guard = Some(PendingRequest {
                        id: request_id.clone(),
                        tx: tx_bytes,
                        label,
                        response: resp_tx,
                      });
                      false
                    }
                  };
                  if request_busy {
                    tracing::warn!(request_id = %request_id, event = "SIGN_REJECTED_BUSY", "another signing request is active");
                    let _ = stream.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                    return;
                  }
                  tracing::info!(request_id = %request_id, event = "SIGN_QUEUED", "signing request accepted");
                  let _ = notify2.send(());
                  // Raise the signature popup; open_in_browser deduplicates an already-live process.
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
                        tracing::warn!(request_id = %request_id, event = "SIGN_TIMED_OUT", "[WalletBridge] signing timed out: {e}");
                      } else if let Ok(Ok(Err(e))) = &other {
                          tracing::info!(request_id = %request_id, event = "SIGN_FAILED", "[WalletBridge] signing rejected: {e}");
                      }
                      // Clear timed-out requests; resolved requests already consume the pending entry.
                      let cleared = {
                        let mut pending = pending2.lock().unwrap();
                        if pending.as_ref().is_some_and(|request| request.id == request_id) {
                          *pending = None;
                          true
                        } else {
                          false
                        }
                      };
                      if cleared {
                        let _ = notify2.send(());
                      }
                      let _ = stream.write_all(&0xFFFF_FFFFu32.to_le_bytes()).await;
                    }
                  }
                });
              }
            }
          });
        });
      }

      // Keep backend defaults aligned with the game client and wallet bridge.
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
      // Kill the reusable hidden wallet popup on exit so its Chrome process cannot linger.
      if let tauri::RunEvent::ExitRequested { .. } = event {
        kill_wallet_popup();
        // Same reasoning for the admin SSH tunnel: an orphaned ssh.exe holds
        // port 8091 and breaks every later tunnel attempt.
        #[cfg(feature = "tournament-admin")]
        kill_admin_tunnel_inner(_app_handle);
      }
    });
}
