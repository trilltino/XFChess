use axum::{http::StatusCode, response::IntoResponse};
use std::path::{Component, Path};

fn asset_path<'a>(prefix: &str, url_path: &'a str) -> Result<&'a str, StatusCode> {
  let relative = url_path
    .strip_prefix(prefix)
    .unwrap_or(url_path)
    .trim_start_matches('/');
  if relative.contains('\\')
    || relative.contains(':')
    || Path::new(relative)
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    return Err(StatusCode::BAD_REQUEST);
  }
  Ok(if relative.contains('.') {
    relative
  } else {
    "index.html"
  })
}

fn response(path: &str, bytes: Vec<u8>) -> axum::response::Response {
  let mime = match Path::new(path)
    .extension()
    .and_then(|extension| extension.to_str())
  {
    Some("html") => "text/html; charset=utf-8",
    Some("js") | Some("mjs") => "application/javascript",
    Some("css") => "text/css",
    Some("svg") => "image/svg+xml",
    Some("png") => "image/png",
    Some("ico") => "image/x-icon",
    Some("woff2") => "font/woff2",
    Some("json") | Some("map") => "application/json",
    Some("wasm") => "application/wasm",
    Some("woff") => "font/woff",
    Some("ttf") => "font/ttf",
    _ => "application/octet-stream",
  };
  axum::response::Response::builder()
    .header("Content-Type", mime)
    .header(
      "Cache-Control",
      if path.ends_with(".html") {
        "no-store"
      } else {
        "public, max-age=31536000, immutable"
      },
    )
    .body(axum::body::Body::from(bytes))
    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(feature = "tournament-admin")]
pub(super) async fn serve_dist_file(
  dist: &Path,
  prefix: &str,
  url_path: &str,
) -> axum::response::Response {
  let path = match asset_path(prefix, url_path) {
    Ok(path) => path,
    Err(status) => return status.into_response(),
  };
  match tokio::fs::read(dist.join(path)).await {
    Ok(bytes) => response(path, bytes),
    Err(_) => StatusCode::NOT_FOUND.into_response(),
  }
}

pub(super) async fn serve_wallet(
  app: &tauri::AppHandle,
  dist: &Path,
  url_path: &str,
  sid: Option<&str>,
) -> axum::response::Response {
  let path = match asset_path("/wallet-ui", url_path) {
    Ok(path) => path,
    Err(status) => return status.into_response(),
  };
  if !cfg!(feature = "custom-protocol") {
    if let Ok(bytes) = tokio::fs::read(dist.join(path)).await {
      return response(path, wallet_title(path, bytes, sid));
    }
  }
  // Check the key first: the resolver otherwise returns index.html for missing assets.
  let resolver = app.asset_resolver();
  if !resolver
    .iter()
    .any(|(key, _)| key.trim_start_matches('/') == path)
  {
    return StatusCode::NOT_FOUND.into_response();
  }
  // The resolver decompresses embedded bytes. Its webview CSP header is not
  // applicable to the system browser's loopback origin.
  match resolver.get_for_scheme(path.to_owned(), false) {
    Some(asset) => response(path, wallet_title(path, asset.bytes, sid)),
    None => StatusCode::NOT_FOUND.into_response(),
  }
}

fn wallet_title(path: &str, bytes: Vec<u8>, sid: Option<&str>) -> Vec<u8> {
  if !path.ends_with(".html") || !sid.is_some_and(super::popup::is_session) {
    return bytes;
  }
  let Some(title) = super::popup::window_title() else {
    return bytes;
  };
  // Native ownership and cleanup must work before React or wallet SDKs load.
  String::from_utf8_lossy(&bytes)
    .replace("<title>XFChess</title>", &format!("<title>{title}</title>"))
    .into_bytes()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rejects_traversal_and_windows_paths() {
    for path in [
      "/wallet-ui/../secret",
      "/wallet-ui/C:/secret",
      "/wallet-ui/a\\secret",
    ] {
      assert_eq!(asset_path("/wallet-ui", path), Err(StatusCode::BAD_REQUEST));
    }
    assert_eq!(asset_path("/wallet-ui", "/wallet-ui/"), Ok("index.html"));
    assert_eq!(
      asset_path("/wallet-ui", "/wallet-ui/assets/app.js"),
      Ok("assets/app.js")
    );
  }
}
