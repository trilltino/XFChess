//! An update is delivered once, the next frame waits for the application's
//! verdict, and a rejection ends the stream without advancing the cursor.

use anyhow::Context as _;
use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use braid_chess::{braid_http::Version, ChessMessage, ChessResource, ChessSubscriber, MovePayload};
use bytes::Bytes;
use futures::StreamExt as _;
use std::time::Duration;
use xfchess_braid_server::resource::protocol::{format_chunk, BraidUpdate};

fn move_chunk(version: u64, fen: &str) -> anyhow::Result<Bytes> {
    let message = ChessMessage::Move(MovePayload::from_uci("e2e4", fen, version as u32, "p"));
    let mut update = BraidUpdate::snapshot(version, serde_json::to_value(message)?);
    update.parents = vec![version - 1];
    Ok(format_chunk(&update))
}

async fn serve() -> anyhow::Result<String> {
    let first = move_chunk(1, "fen1")?;
    let second = move_chunk(2, "fen2")?;
    let handler = get(move || {
        let (first, second) = (first.clone(), second.clone());
        async move {
            let frames = futures::stream::iter([first, second])
                .map(Ok::<Bytes, std::io::Error>)
                .chain(futures::stream::pending());
            Response::builder()
                .status(209)
                .header("Content-Type", "application/http-history")
                .body(Body::from_stream(frames))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    let router = Router::new().route("/game/g1/moves", handler);
    tokio::spawn(async move { axum::serve(listener, router).await });
    Ok(url)
}

#[tokio::test]
async fn the_next_update_waits_for_the_application() -> anyhow::Result<()> {
    let url = serve().await?;
    let subscriber = ChessSubscriber::new(&url, "g1")?;
    let (deliveries, _task) = subscriber
        .subscribe_acknowledged(ChessResource::moves("g1"), Vec::new())
        .await?;

    let first = tokio::time::timeout(Duration::from_secs(3), deliveries.recv())
        .await?
        .context("stream ended")?;
    assert_eq!(first.receipt.versions(), [Version::new("1")]);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), deliveries.recv())
            .await
            .is_err(),
        "the second update must not be released before the first is acknowledged"
    );

    first.receipt.acknowledge()?;
    let second = tokio::time::timeout(Duration::from_secs(3), deliveries.recv())
        .await?
        .context("stream ended")?;
    assert_eq!(second.receipt.versions(), [Version::new("2")]);
    Ok(())
}

#[tokio::test]
async fn rejecting_an_update_ends_the_stream() -> anyhow::Result<()> {
    let url = serve().await?;
    let subscriber = ChessSubscriber::new(&url, "g1")?;
    let (deliveries, task) = subscriber
        .subscribe_acknowledged(ChessResource::moves("g1"), Vec::new())
        .await?;

    let first = tokio::time::timeout(Duration::from_secs(3), deliveries.recv())
        .await?
        .context("stream ended")?;
    // Dropping a receipt without acknowledging it is a rejection.
    drop(first);
    tokio::time::timeout(Duration::from_secs(3), task).await??;
    assert!(
        deliveries.recv().await.is_err(),
        "no further update may follow a rejection"
    );
    Ok(())
}
