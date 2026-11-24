use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

mod assets;
mod http;
mod lifecycle;
#[cfg(windows)]
mod native;
mod popup;
#[cfg(windows)]
mod process;
mod proxy;
mod requests;
mod tcp;
pub(crate) use http::http_server;
pub(crate) use lifecycle::attach_game;
pub(crate) use lifecycle::shutdown;
pub(crate) use popup::{open_wallet_popup, spawn_wallet_popup_idle_reaper};
pub(crate) use tcp::start_tcp_bridge;
#[derive(Default, Clone)]
pub(super) struct WalletPubkey(pub(super) Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
pub(super) struct WalletUsername(pub(super) Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
pub(super) struct WalletProvider(pub(super) Arc<Mutex<Option<String>>>);

#[derive(Default, Clone)]
pub(super) struct WalletJwt(pub(super) Arc<Mutex<Option<String>>>);

pub(super) struct PendingRequest {
  id: String,
  tx: Vec<u8>,
  label: String,
  kind: &'static str,
  response: oneshot::Sender<Result<Vec<u8>, String>>,
}

pub(super) type PendingTxInner = Option<PendingRequest>;
pub(super) type PendingTx = Arc<Mutex<PendingTxInner>>;

pub(super) type PendingTxNotify = tokio::sync::watch::Sender<()>;

pub(super) const SIGN_TIMEOUT_SECS: u64 = 120;

pub(super) static ACTUAL_HTTP_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

pub(super) fn nominal_http_port() -> u16 {
  std::env::var("XFCHESS_WALLET_PORT")
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(7454)
}

pub(super) fn http_port() -> u16 {
  ACTUAL_HTTP_PORT
    .get()
    .copied()
    .unwrap_or_else(nominal_http_port)
}

pub(super) fn http_bridge_port_file() -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-http-{}.port", nominal_http_port()))
}

pub(super) async fn bind_http_port() -> Option<(TcpListener, u16)> {
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

pub(super) fn wallet_bridge_port_file(base_port: u16) -> std::path::PathBuf {
  std::env::temp_dir().join(format!("xfchess-wallet-bridge-{base_port}.port"))
}

fn remove_port_files() {
  for (path, expected) in [
    (http_bridge_port_file(), Some(http_port())),
    (
      wallet_bridge_port_file(http_port()),
      TCP_PORT.get().copied(),
    ),
  ] {
    if expected.is_some()
      && std::fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        == expected
    {
      let _ = std::fs::remove_file(path);
    }
  }
}

static TCP_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

pub(super) static BACKEND_URL_OVERRIDE: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn backend_url_override_cell() -> &'static Mutex<Option<String>> {
  &BACKEND_URL_OVERRIDE
}

pub(super) fn get_backend_url() -> String {
  if let Some(url) = backend_url_override_cell().lock().clone() {
    return url;
  }
  std::env::var("SIGNING_SERVICE_URL")
    .or_else(|_| std::env::var("BACKEND_URL"))
    .unwrap_or_else(|_| "https://xfchess.com".to_string())
}

pub(super) fn set_backend_url_override(url: String) {
  let mut cell = backend_url_override_cell().lock();
  let changed = cell.as_deref() != Some(url.as_str());
  if changed {
    tracing::info!("[Backend] target set by game client: {url}");
  }
  *cell = Some(url);
}

pub(super) fn instance_cache_dir() -> PathBuf {
  dirs::data_local_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("xfchess")
    .join(format!("port-{}", http_port()))
}

pub(super) fn consent_path() -> PathBuf {
  instance_cache_dir().join("consent.json")
}

fn allowed_origin(origin: &str) -> bool {
  if origin == "tauri://localhost" {
    return true;
  }
  let Ok(url) = reqwest::Url::parse(origin) else {
    return false;
  };
  if !url.username().is_empty()
    || url.password().is_some()
    || url.path() != "/"
    || url.query().is_some()
    || url.fragment().is_some()
  {
    return false;
  }
  match (url.scheme(), url.host_str()) {
    ("http" | "https", Some("tauri.localhost")) => true,
    ("http", Some("localhost" | "127.0.0.1")) => {
      let port = url.port_or_known_default();
      port == Some(http_port()) || port == Some(5174) || port == Some(5175) || port == Some(1420)
    }
    _ => false,
  }
}

fn valid_backend_url(value: &str) -> bool {
  reqwest::Url::parse(value).is_ok_and(|url| {
    url.username().is_empty()
      && url.password().is_none()
      && url.query().is_none()
      && url.fragment().is_none()
      && url.host_str().is_some()
      && (url.scheme() == "https"
        || (url.scheme() == "http"
          && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn origins_are_exact_and_reject_spoofed_hosts() {
    assert!(allowed_origin("http://127.0.0.1:7454"));
    assert!(allowed_origin("http://localhost:5174"));
    assert!(allowed_origin("tauri://localhost"));
    for origin in [
      "http://tauri.localhost.evil.com",
      "https://localhost:7454",
      "http://localhost:9999",
      "http://localhost:7454@evil.com",
      "null",
    ] {
      assert!(!allowed_origin(origin), "{origin}");
    }
  }
}
