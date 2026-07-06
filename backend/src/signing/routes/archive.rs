use crate::signing::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Json, Response},
    routing::get,
    Router,
};
use serde_json::json;
use std::fs;
use std::path::Path;

const ARCHIVE_PATH: &str = "archive/games.xfg";
const WALLET_INDEX_PATH: &str = "archive/wallets.idx";

pub fn archive_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/archive/stats", get(get_archive_stats))
        .route("/admin/archive/download/games", get(download_games_archive))
        .route(
            "/admin/archive/download/wallets",
            get(download_wallets_index),
        )
}

async fn get_archive_stats(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let games_size = fs::metadata(ARCHIVE_PATH).map(|m| m.len()).unwrap_or(0);
    let wallets_size = fs::metadata(WALLET_INDEX_PATH)
        .map(|m| m.len())
        .unwrap_or(0);

    let wallet_count = if Path::new(WALLET_INDEX_PATH).exists() {
        fs::read_to_string(WALLET_INDEX_PATH)
            .map(|c| c.lines().count())
            .unwrap_or(0)
    } else {
        0
    };

    Ok(Json(json!({
        "games_archive_size_bytes": games_size,
        "wallets_index_size_bytes": wallets_size,
        "unique_wallets_count": wallet_count,
    })))
}

async fn download_games_archive() -> Result<Response, StatusCode> {
    if !Path::new(ARCHIVE_PATH).exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = fs::read(ARCHIVE_PATH).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"games.xfg\"",
        )
        .body(axum::body::Body::from(content))
        .unwrap())
}

async fn download_wallets_index() -> Result<Response, StatusCode> {
    if !Path::new(WALLET_INDEX_PATH).exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = fs::read(WALLET_INDEX_PATH).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"wallets.idx\"",
        )
        .body(axum::body::Body::from(content))
        .unwrap())
}
