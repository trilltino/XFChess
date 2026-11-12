#![cfg(feature = "coverage")]

mod common;

use anyhow::Context as _;

/// Inspects the constructed Axum trie without executing application handlers.
/// This is inventory only and is deliberately not behavior coverage.
#[tokio::test]
async fn export_real_backend_route_inventory() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let router = app.router();
    let mut rows = backend::coverage::runtime_routes(&router).map_err(anyhow::Error::msg)?;
    // nest_service hides the child's trie behind an opaque Service. Inventory
    // that actual child router too, so new Braid endpoints cannot evade the gate.
    rows.retain(|(method, path)| !(method == "ANY" && path.starts_with("/braid")));
    let braid = xfchess_braid_server::braid_router((*app.state.braid_hub).clone());
    for (method, path) in backend::coverage::runtime_routes(&braid).map_err(anyhow::Error::msg)? {
        rows.insert((method, format!("/braid{path}")));
    }
    anyhow::ensure!(
        rows.len() > 150,
        "runtime route inventory unexpectedly shrank"
    );
    let out = std::env::var_os("XFCHESS_COVERAGE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/coverage/application/surfaces")
        });
    std::fs::create_dir_all(&out)?;
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(method, path)| serde_json::json!({"method": method, "path": path}))
        .collect();
    std::fs::write(
        out.join("backend-routes.json"),
        serde_json::to_vec_pretty(&rows)?,
    )
    .context("write backend inventory")?;
    Ok(())
}
