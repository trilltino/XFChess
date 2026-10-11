use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

type Counters = BTreeMap<(String, String, String), u64>;
static REQUESTS: OnceLock<Mutex<Counters>> = OnceLock::new();

pub struct RpcMetrics;

pub fn register(url: &str) {
    let provider = solana_client::client_error::reqwest::Url::parse(url)
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|_| "invalid".into());
    if let Ok(mut counters) = REQUESTS.get_or_init(|| Mutex::new(BTreeMap::new())).lock() {
        counters
            .entry((provider, "none".into(), "none".into()))
            .or_default();
    }
}

#[async_trait::async_trait]
impl reqwest_middleware::Middleware for RpcMetrics {
    async fn handle(
        &self,
        request: solana_client::client_error::reqwest::Request,
        extensions: &mut axum::http::Extensions,
        next: reqwest_middleware::Next<'_>,
    ) -> reqwest_middleware::Result<solana_client::client_error::reqwest::Response> {
        let provider = request.url().origin().ascii_serialization();
        let method = request
            .body()
            .and_then(|body| body.as_bytes())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
            .and_then(|value| {
                value
                    .get("method")
                    .and_then(|method| method.as_str())
                    .map(str::to_owned)
            })
            .filter(|method| {
                method.len() <= 64
                    && method
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
            .unwrap_or_else(|| "other".into());
        let result = next.run(request, extensions).await;
        let status = result
            .as_ref()
            .map(|response| response.status().as_u16().to_string())
            .unwrap_or_else(|_| "transport_error".into());
        if let Ok(mut counters) = REQUESTS.get_or_init(|| Mutex::new(BTreeMap::new())).lock() {
            let counter = counters.entry((provider, method, status)).or_default();
            *counter = counter.saturating_add(1);
        }
        result
    }
}

pub fn render_prometheus() -> String {
    let mut output = "# TYPE xfchess_rpc_http_requests_total counter\n".to_string();
    if let Ok(counters) = REQUESTS.get_or_init(|| Mutex::new(BTreeMap::new())).lock() {
        for ((provider, method, status), value) in counters.iter() {
            output.push_str(&format!("xfchess_rpc_http_requests_total{{provider=\"{provider}\",method=\"{method}\",status=\"{status}\"}} {value}\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn sdk_retries_are_counted_without_exposing_credentials() {
        let calls = Arc::new(AtomicU32::new(0));
        let observed = calls.clone();
        let app = axum::Router::new().route(
            "/secret-token",
            axum::routing::post(move || {
                let observed = observed.clone();
                async move {
                    if observed.fetch_add(1, Ordering::Relaxed) == 0 {
                        (
                            axum::http::StatusCode::TOO_MANY_REQUESTS,
                            [("Retry-After", "0")],
                            axum::Json(serde_json::json!({"error":"rate limit"})),
                        )
                    } else {
                        (
                            axum::http::StatusCode::OK,
                            [("Retry-After", "0")],
                            axum::Json(serde_json::json!({"jsonrpc":"2.0","id":0,"result":42})),
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = crate::signing::solana::rpc::make_async_rpc(&format!(
            "{origin}/secret-token?api-key=private"
        ));
        assert_eq!(client.get_slot().await.unwrap(), 42);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        let metrics = render_prometheus();
        assert!(metrics.contains(&format!(
            "provider=\"{origin}\",method=\"getSlot\",status=\"429\"}} 1"
        )));
        assert!(metrics.contains(&format!(
            "provider=\"{origin}\",method=\"getSlot\",status=\"200\"}} 1"
        )));
        assert!(!metrics.contains("secret-token") && !metrics.contains("api-key"));
        server.abort();
    }
}
