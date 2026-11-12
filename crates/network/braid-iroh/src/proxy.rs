use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use iroh::{Endpoint, EndpointId};
use iroh_h3_client::IrohH3Client;
use tokio::net::TcpListener;
use tower::ServiceExt;

#[derive(Clone)]
struct ProxyState {
    client: IrohH3Client,
    default_peer: EndpointId,
    local_id: EndpointId,
    local_router: Option<Router>,
}

pub(crate) async fn serve(
    endpoint: Endpoint,
    listener: TcpListener,
    default_peer: EndpointId,
    local_router: Router,
) -> anyhow::Result<()> {
    let local_id = endpoint.id();
    let local_router = (default_peer == local_id).then_some(local_router);
    let state = ProxyState {
        client: IrohH3Client::new(endpoint, crate::node::BRAID_H3_ALPN.to_vec()),
        default_peer,
        local_id,
        local_router,
    };
    let router = Router::new()
        .route("/", any(proxy_handler))
        .route("/{*path}", any(proxy_handler))
        .with_state(state);
    axum::serve(listener, router).await?;
    Ok(())
}

fn hop_header(name: &HeaderName, headers: &HeaderMap) -> bool {
    matches!(
        name.as_str(),
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) || headers
        .get_all("connection")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case(name.as_str()))
}

async fn proxy_handler(
    State(state): State<ProxyState>,
    request: Request,
) -> Result<Response, StatusCode> {
    if let Some(router) = state.local_router {
        let (mut parts, body) = request.into_parts();
        // Route parameters from the bridge must not reach the resource router.
        parts.extensions.clear();
        // The loopback bridge speaks as this node; remote peers never reach this branch.
        parts
            .extensions
            .insert(iroh_h3_axum::RemoteId(state.local_id));
        let request = Request::from_parts(parts, body);
        return router
            .oneshot(request)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    let path = request
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let target = format!("https://{}{path}", state.default_peer);
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let mut builder = state.client.request(parts.method, &target);
    for (name, value) in &parts.headers {
        if !hop_header(name, &parts.headers) {
            builder = builder.header(name, value);
        }
    }
    let response = builder
        .bytes(body)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("TCP bridge upstream failed: {e}");
            StatusCode::BAD_GATEWAY
        })?;
    let mut builder = Response::builder().status(response.status);
    for (name, value) in &response.headers {
        if !hop_header(name, &response.headers) {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
