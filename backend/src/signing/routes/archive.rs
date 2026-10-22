use crate::signing::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Json, Response},
    routing::get,
    Router,
};
use futures::StreamExt;
use serde_json::json;
use tokio::io::AsyncReadExt;

// Must match tasks::archiver; production permits writes only under data/.
const ARCHIVE_PATH: &str = "data/archive/games.xfg";
const WALLET_INDEX_PATH: &str = "data/archive/wallets.idx";

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
    let games_size = tokio::fs::metadata(ARCHIVE_PATH)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let wallets_size = tokio::fs::metadata(WALLET_INDEX_PATH)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut wallet_count = 0;
    if let Ok(file) = tokio::fs::File::open(WALLET_INDEX_PATH).await {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut lines = BufReader::new(file).lines();
        while lines
            .next_line()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some()
        {
            wallet_count += 1;
        }
    }

    Ok(Json(json!({
        "games_archive_size_bytes": games_size,
        "wallets_index_size_bytes": wallets_size,
        "unique_wallets_count": wallet_count,
    })))
}

async fn download_games_archive() -> Result<Response, StatusCode> {
    download(
        ARCHIVE_PATH,
        "application/octet-stream",
        "attachment; filename=\"games.xfg\"",
    )
    .await
}

async fn download_wallets_index() -> Result<Response, StatusCode> {
    download(
        WALLET_INDEX_PATH,
        "text/plain",
        "attachment; filename=\"wallets.idx\"",
    )
    .await
}

async fn download(
    path: &str,
    content_type: &'static str,
    disposition: &'static str,
) -> Result<Response, StatusCode> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;
    let stream = async_stream::try_stream! {
        loop {
            let mut chunk = vec![0u8; 64 * 1024];
            let count = file.read(&mut chunk).await?;
            if count == 0 { break; }
            chunk.truncate(count);
            yield bytes::Bytes::from(chunk);
        }
    };
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(axum::body::Body::from_stream(
            stream.map(|chunk: Result<bytes::Bytes, std::io::Error>| chunk),
        ))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
