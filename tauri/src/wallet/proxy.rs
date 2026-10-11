use axum::{http::StatusCode, response::IntoResponse, Json};

pub(super) fn backend_client() -> &'static reqwest::Client {
  static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
  CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .timeout(std::time::Duration::from_secs(15))
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .expect("wallet backend HTTP client")
  })
}

pub(super) fn backend_unreachable_msg(e: reqwest::Error) -> String {
  tracing::warn!("[HTTP] backend request failed: {e}");
  "Could not reach the backend service. Please check it's running and try again.".to_string()
}
pub(super) fn backend_bad_response_msg(e: reqwest::Error) -> String {
  tracing::warn!("[HTTP] backend returned a non-JSON response: {e}");
  "The backend returned an unexpected response. Please try again in a moment.".to_string()
}

// Read the body once. Preserve its status and return JSON or plain text
// without discarding backend diagnostics.
pub(super) async fn forward_backend_response(resp: reqwest::Response) -> axum::response::Response {
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
pub(super) fn forward_client_headers(
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

pub(super) async fn proxy_post(
  url: &str,
  body: serde_json::Value,
  headers: &axum::http::HeaderMap,
) -> axum::response::Response {
  let client = backend_client();
  let req = forward_client_headers(client.post(url).json(&body), headers);
  match req.send().await {
    Ok(resp) => forward_backend_response(resp).await,
    Err(e) => (StatusCode::BAD_GATEWAY, backend_unreachable_msg(e)).into_response(),
  }
}

