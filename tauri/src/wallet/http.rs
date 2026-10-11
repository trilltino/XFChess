#[cfg(feature = "tournament-admin")]
use super::assets::serve_dist_file;
use super::popup::{hide_wallet_popup, mark_session_ready, open_wallet_popup_with_step};
use super::proxy::{
  backend_client, backend_unreachable_msg, forward_backend_response, forward_client_headers,
  proxy_post,
};
use super::*;
use axum::http::{Method, StatusCode};
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as B64, Engine};

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
  #[cfg(feature = "tournament-admin")]
  dist_path: std::path::PathBuf,
  wallet_ui_dist_path: std::path::PathBuf,
  needs_profile_step: Arc<std::sync::atomic::AtomicBool>,
}

fn pending_json(pending: &PendingTx) -> serde_json::Value {
  let lock = pending.lock();
  let tx_b64 = lock.as_ref().map(|request| B64.encode(&request.tx));
  let label = lock.as_ref().map(|request| request.label.clone());
  let request_id = lock.as_ref().map(|request| request.id.clone());
  let kind = lock.as_ref().map(|request| request.kind);
  serde_json::json!({ "tx": tx_b64, "label": label, "request_id": request_id, "kind": kind, "step": super::popup::current_step() })
}

// Plain-fetch fallback for SSE: return {"tx":"<b64>","label":"<str>"} or {"tx":null}.
async fn get_pending(State(s): State<LocalState>) -> impl IntoResponse {
  Json(pending_json(&s.pending))
}

// SSE emits current pending state on connect and whenever it changes.
async fn get_pending_stream(
  State(s): State<LocalState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
  let rx = s.notify.subscribe();
  let pending = s.pending;
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
  let result = if signed_b64.is_empty() {
    Err("User cancelled".into())
  } else if signed_b64.len() > 90_000 {
    return StatusCode::PAYLOAD_TOO_LARGE;
  } else {
    match B64.decode(&signed_b64) {
      Ok(bytes) if !bytes.is_empty() && bytes.len() <= 64 * 1024 => Ok(bytes),
      _ => return StatusCode::BAD_REQUEST,
    }
  };
  if super::requests::finish(&s.pending, &s.notify, request_id, result) {
    StatusCode::OK
  } else {
    StatusCode::CONFLICT
  }
}

// POST /cancel — wallet-ui posts {"request_id":"...","reason":"..."}
async fn post_cancel(
  State(s): State<LocalState>,
  Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
  let request_id = body["request_id"].as_str().unwrap_or("");
  let reason = body["reason"].as_str().unwrap_or("Cancelled by wallet UI");
  if super::requests::finish(&s.pending, &s.notify, request_id, Err(reason.into())) {
    StatusCode::OK
  } else {
    StatusCode::CONFLICT
  }
}

// An empty username explicitly clears the cache; an absent field leaves it
// unchanged. Keep these distinct when wallets share a browser profile.
async fn post_wallet(
  State(s): State<LocalState>,
  Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
  if let Some(pk) = body["pubkey"].as_str() {
    *s.wallet_pubkey.0.lock() = Some(pk.to_string());
    if let Some(username) = body.get("username").and_then(|v| v.as_str()) {
      *s.wallet_username.0.lock() = if username.is_empty() {
        None
      } else {
        Some(username.to_string())
      };
    }
    // Absent means "caller has no opinion" — leave whatever is cached alone,
    // same convention as `username` above.
    if let Some(provider) = body.get("provider").and_then(|v| v.as_str()) {
      *s.wallet_provider.0.lock() = if provider.is_empty() {
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
  *s.wallet_pubkey.0.lock() = None;
  *s.wallet_username.0.lock() = None;
  *s.wallet_provider.0.lock() = None;
  *s.wallet_jwt.0.lock() = None;
  if let Some(request) = s.pending.lock().take() {
    let _ = request.response.send(Err("Wallet disconnected".into()));
  }
  s.notify.send_replace(());
  tracing::info!("[HTTP] Wallet disconnected and bridge auth state cleared");
  StatusCode::OK
}

// Hide resolved signing popups for reuse; the idle reaper later closes them.
async fn post_hide(State(s): State<LocalState>) -> impl IntoResponse {
  let pending = s.pending.lock();
  if pending.is_some() {
    return StatusCode::CONFLICT;
  }
  hide_wallet_popup();
  drop(pending);
  StatusCode::OK
}

async fn post_close(State(s): State<LocalState>) -> impl IntoResponse {
  let mut pending = s.pending.lock();
  if let Some(request) = pending.take() {
    let _ = request.response.send(Err("Wallet window closed".into()));
  }
  s.notify.send_replace(());
  super::popup::kill_wallet_popup();
  drop(pending);
  StatusCode::OK
}

async fn get_status(
  State(s): State<LocalState>,
  headers: axum::http::HeaderMap,
) -> impl IntoResponse {
  if let Some(pid) = headers
    .get("X-Game-Pid")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.parse().ok())
  {
    if let Err(error) =
      super::lifecycle::attach_game(pid, s.app.clone(), s.pending.clone(), s.notify.clone())
    {
      tracing::warn!(%error, "Game attachment rejected");
      return (StatusCode::CONFLICT, error).into_response();
    }
  }
  let pubkey = s.wallet_pubkey.0.lock().clone();
  let username = s.wallet_username.0.lock().clone();
  let provider = s.wallet_provider.0.lock().clone();
  Json(serde_json::json!({
    "bridge_pid": std::process::id(),
    "connected": pubkey.is_some(),
    "pubkey": pubkey,
    "username": username,
    "provider": provider,
  }))
  .into_response()
}

async fn open_wallet(State(s): State<LocalState>) -> impl IntoResponse {
  if s.pending.lock().is_some() {
    return StatusCode::CONFLICT;
  }
  super::popup::open_wallet_popup(&s.app);
  StatusCode::OK
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
    *s.wallet_jwt.0.lock() = Some(token.to_string());
    tracing::info!("[HTTP] JWT stored via /token");
  }
  StatusCode::OK
}

// GET /token — game client polls this to retrieve the JWT after wallet-ui auth
async fn get_token(State(s): State<LocalState>) -> impl IntoResponse {
  let token = s.wallet_jwt.0.lock().clone();
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
  let client = backend_client();
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
  let client = backend_client();
  let url = format!("{}/api/auth/username", get_backend_url());
  let req = forward_client_headers(client.patch(&url).json(&body), &headers);
  match req.send().await {
    Ok(resp) => {
      // Mirror a confirmed rename into bridge state for the game status poller.
      if resp.status().is_success() {
        if let Some(username) = body["username"].as_str() {
          if !username.is_empty() {
            *s.wallet_username.0.lock() = Some(username.to_string());
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
  let client = backend_client();
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

async fn api_simulate_tx(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
  let Some(tx) = body["tx_b64"]
    .as_str()
    .filter(|value| value.len() <= 90_000)
  else {
    return StatusCode::BAD_REQUEST.into_response();
  };
  let rpc_body = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "simulateTransaction",
    "params": [tx, { "encoding": "base64", "sigVerify": false, "commitment": "confirmed" }] });
  match backend_client()
    .post(format!("{}/api/rpc", get_backend_url()))
    .timeout(std::time::Duration::from_secs(15))
    .json(&rpc_body)
    .send()
    .await
  {
    Ok(response) => forward_backend_response(response).await,
    Err(error) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(error)).into_response(),
  }
}

// Use the backend URL resolved by the game so bridge and client share one endpoint.
async fn api_set_backend_url(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
  match body["url"].as_str() {
    Some(url) if super::valid_backend_url(url) => {
      set_backend_url_override(url.to_string());
      StatusCode::OK
    }
    _ => StatusCode::BAD_REQUEST,
  }
}

async fn api_get_backend_url() -> impl IntoResponse {
  Json(serde_json::json!({
    "url": get_backend_url(),
    "explicit": backend_url_override_cell().lock().is_some(),
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
  if s.pending.lock().is_some() {
    return StatusCode::CONFLICT;
  }
  s.needs_profile_step
    .store(true, std::sync::atomic::Ordering::Relaxed);
  open_wallet_popup_with_step(Some("profile"));
  s.notify.send_replace(());
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
      *s.wallet_username.0.lock() = Some(username.to_string());
    }
  }
  StatusCode::OK
}

// Generic passthrough for remaining /api/** calls to backend
async fn api_check_wallet(
  axum::extract::Path(pubkey): axum::extract::Path<String>,
  headers: axum::http::HeaderMap,
) -> impl IntoResponse {
  let client = backend_client();
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
  let sid = uri
    .query()
    .and_then(|query| query.split('&').find_map(|part| part.strip_prefix("sid=")));
  super::assets::serve_wallet(&s.app, &s.wallet_ui_dist_path, uri.path(), sid).await
}

pub(crate) async fn http_server(listener: TcpListener, app: tauri::AppHandle) {
  use tauri::Manager;
  let pending = app.state::<PendingTx>().inner().clone();
  let notify = app.state::<PendingTxNotify>().inner().clone();
  let wallet_pubkey = app.state::<WalletPubkey>().inner().clone();
  let wallet_username = app.state::<WalletUsername>().inner().clone();
  let wallet_provider = app.state::<WalletProvider>().inner().clone();
  let wallet_jwt = app.state::<WalletJwt>().inner().clone();
  // Reflect only local/webview origins to prevent arbitrary websites reading bridge tokens.
  let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin, _parts| {
      super::allowed_origin(origin.to_str().unwrap_or(""))
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

  let state = LocalState {
    app,
    pending,
    notify,
    wallet_pubkey,
    wallet_username,
    wallet_provider,
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
    .route("/cancel", post(post_cancel))
    .route("/wallet", post(post_wallet))
    .route("/wallet/disconnect", post(post_wallet_disconnect))
    .route("/hide", post(post_hide))
    .route("/close", post(post_close))
    .route("/status", get(get_status))
    .route("/api/wallet/open", post(open_wallet))
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
    .route("/api/simulate-tx", post(api_simulate_tx))
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

  let router = router
    .layer(axum::middleware::from_fn(
      |request: axum::extract::Request, next: axum::middleware::Next| async move {
        let host = request
          .headers()
          .get(axum::http::header::HOST)
          .and_then(|value| value.to_str().ok())
          .unwrap_or("");
        if host != format!("localhost:{}", http_port())
          && host != format!("127.0.0.1:{}", http_port())
        {
          return StatusCode::FORBIDDEN.into_response();
        }
        if request
          .headers()
          .get(axum::http::header::ORIGIN)
          .is_some_and(|origin| !super::allowed_origin(origin.to_str().unwrap_or("")))
        {
          return StatusCode::FORBIDDEN.into_response();
        }
        let path = request.uri().path();
        if matches!(
          path,
          "/pending"
            | "/pending/stream"
            | "/resolved"
            | "/cancel"
            | "/wallet"
            | "/wallet/disconnect"
            | "/hide"
            | "/close"
        ) || (request.headers().contains_key(axum::http::header::ORIGIN)
          && (path.starts_with("/api/") || path == "/token" || path == "/status"))
        {
          let query_sid = request
            .uri()
            .query()
            .and_then(|query| query.split('&').find_map(|part| part.strip_prefix("sid=")));
          let sid = request
            .headers()
            .get("X-Session-Id")
            .and_then(|value| value.to_str().ok())
            .or(query_sid)
            .unwrap_or("");
          if !super::popup::is_session(sid) {
            return StatusCode::FORBIDDEN.into_response();
          }
        }
        next.run(request).await
      },
    ))
    .layer(cors)
    .with_state(state);

  tracing::info!(port = http_port(), "Wallet HTTP bridge listening");
  if let Err(error) = axum::serve(listener, router).await {
    tracing::error!(%error, "Wallet HTTP bridge failed");
  }
}
