//! Test instrumentation shared by the backend and the native wallet bridge.
//! No request bodies, credentials or concrete URL parameters are recorded.

use axum::{extract::MatchedPath, middleware::Next, response::Response, Router};
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;

/// Extract explicit methods and paths from the constructed runtime trie. Axum's
/// automatic HEAD fallback for GET is not a separately registered entry point.
/// Axum 0.8 has no public route iterator. Fail closed if its Debug format changes.
pub fn runtime_routes<S>(router: &Router<S>) -> Result<BTreeSet<(String, String)>, String> {
    let debug = format!("{router:?}");
    let (primary, _) = debug
        .split_once(", fallback_router:")
        .ok_or("missing fallback router")?;
    let (endpoints, paths) = primary
        .split_once("}, node: Node { paths: {")
        .ok_or("Axum route inventory format changed")?;
    let mut methods = std::collections::BTreeMap::new();
    for entry in endpoints.split("RouteId(").skip(1) {
        let (id, endpoint) = entry.split_once("): ").ok_or("invalid endpoint ID")?;
        let mut found = Vec::new();
        if endpoint.starts_with("MethodRouter(") {
            for method in [
                "get", "head", "post", "put", "patch", "delete", "options", "trace", "connect",
            ] {
                let field = format!("{method}: ");
                let (_, value) = endpoint
                    .split_once(&field)
                    .ok_or("missing Axum method field")?;
                if !value.starts_with("None") {
                    found.push(method.to_uppercase());
                }
            }
        } else if endpoint.starts_with("Route(") {
            found.push("ANY".into());
        } else {
            return Err("unknown Axum endpoint kind".into());
        }
        if found.is_empty() {
            return Err("endpoint has no explicit methods; review its fallback".into());
        }
        methods.insert(id.to_owned(), found);
    }
    let mut routes = BTreeSet::new();
    for entry in paths.split("RouteId(").skip(1) {
        let (id, quoted) = entry.split_once("): \"").ok_or("invalid path ID")?;
        let (path, _) = quoted.split_once('"').ok_or("invalid quoted path")?;
        let path = path.replace("__private__axum_nest_tail_param", "path");
        for method in methods.get(id).ok_or("path missing method registration")? {
            routes.insert((method.clone(), path.clone()));
        }
    }
    if routes.is_empty() {
        return Err("empty runtime route inventory".into());
    }
    Ok(routes)
}

/// Records real test traffic only when the coverage runner supplies an output
/// directory. Inventory probes use their own router and never count as hits.
pub async fn record_hit(request: axum::extract::Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned());
    let method = request.method().to_string();
    let response = next.run(request).await;
    if let (Some(dir), Some(path)) = (std::env::var_os("XFCHESS_COVERAGE_DIR"), path) {
        let thread = std::thread::current();
        let row = serde_json::json!({
            "method": method, "path": path,
            "status": response.status().as_u16(),
            "test": thread.name().unwrap_or("unnamed"),
        });
        if let Err(error) = append_hit(Path::new(&dir), &row) {
            // Broken instrumentation must not produce a successful coverage run.
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
        }
    }
    response
}

fn append_hit(dir: &Path, row: &serde_json::Value) -> Result<(), String> {
    static WRITER: Mutex<()> = Mutex::new(());
    let _guard = WRITER.lock().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("hits-{}.jsonl", std::process::id())))
        .map_err(|e| e.to_string())?;
    writeln!(file, "{row}").map_err(|e| e.to_string())
}

use axum::response::IntoResponse as _;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_inventory_keeps_nested_aliases_methods_and_catchalls() -> Result<(), String> {
        let child = Router::new().route("/{id}", axum::routing::get(|| async {}).post(|| async {}));
        let router: Router = Router::new()
            .nest("/items", child.clone())
            .nest("/api/items", child)
            .route("/assets/{*path}", axum::routing::get(|| async {}))
            .nest_service(
                "/service",
                Router::new().route("/", axum::routing::get(|| async {})),
            );
        let rows = runtime_routes(&router)?;
        for (method, path) in [
            ("GET", "/items/{id}"),
            ("POST", "/items/{id}"),
            ("GET", "/api/items/{id}"),
            ("POST", "/api/items/{id}"),
            ("GET", "/assets/{*path}"),
            ("ANY", "/service"),
            ("ANY", "/service/"),
            ("ANY", "/service/{*path}"),
        ] {
            assert!(
                rows.contains(&(method.into(), path.into())),
                "missing {method} {path}: {rows:?}"
            );
        }
        assert_eq!(rows.len(), 8);
        Ok(())
    }

    #[test]
    fn empty_inventory_is_an_error() {
        assert!(runtime_routes(&Router::<()>::new()).is_err());
    }
}
