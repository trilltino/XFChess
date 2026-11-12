//! Stream independence: a failing chat stream must never disturb moves.

use anyhow::Context as _;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use braid_chess::{
    braid_http::Version, ChatPayload, ChessMessage, ChessResource, MovePayload, StreamCursor,
    StreamHandle, StreamState, SupervisorConfig,
};
use bytes::Bytes;
use futures::StreamExt as _;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use xfchess_braid_server::resource::protocol::{format_chunk, BraidUpdate};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatMode {
    /// Every request is refused with 503.
    Refuse,
    /// One update, then the server closes the stream.
    CloseAfterOne,
    /// 410 on the first request, then a held-open stream.
    GoneOnce,
}

struct Stub {
    chat_mode: ChatMode,
    chat_requests: AtomicU32,
    moves_requests: AtomicU32,
    parents: Mutex<Vec<(String, Option<String>)>>,
}

fn chunk(version: u64, message: &ChessMessage) -> anyhow::Result<Bytes> {
    let body = serde_json::to_value(message)?;
    let mut update = BraidUpdate::snapshot(version, body);
    update.parents = vec![version - 1];
    Ok(format_chunk(&update))
}

fn stream_response(first: Bytes, hold_open: bool) -> Response {
    let head = futures::stream::once(async move { Ok::<Bytes, std::io::Error>(first) });
    let body = if hold_open {
        Body::from_stream(head.chain(futures::stream::pending()))
    } else {
        Body::from_stream(head)
    };
    Response::builder()
        .status(209)
        .header("Content-Type", "application/http-history")
        .header("Subscribe", "true")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn handler(
    State(stub): State<Arc<Stub>>,
    Path((_game, stream)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let parents = headers
        .get("parents")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if let Ok(mut seen) = stub.parents.lock() {
        seen.push((stream.clone(), parents));
    }
    if stream == "moves" {
        stub.moves_requests.fetch_add(1, Ordering::SeqCst);
        let message = ChessMessage::Move(MovePayload::from_uci("e2e4", "fen1", 1, "p"));
        return match chunk(1, &message) {
            Ok(bytes) => stream_response(bytes, true),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    }
    let attempt = stub.chat_requests.fetch_add(1, Ordering::SeqCst);
    let message = ChessMessage::Chat(ChatPayload {
        player: "bob".into(),
        text: "hi".into(),
        timestamp_ms: 1,
    });
    match stub.chat_mode {
        ChatMode::Refuse => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        ChatMode::CloseAfterOne => match chunk(1, &message) {
            Ok(bytes) => stream_response(bytes, false),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        ChatMode::GoneOnce if attempt == 0 => StatusCode::GONE.into_response(),
        ChatMode::GoneOnce => match chunk(8, &message) {
            Ok(bytes) => stream_response(bytes, true),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
    }
}

async fn serve(chat_mode: ChatMode) -> anyhow::Result<(String, Arc<Stub>)> {
    let stub = Arc::new(Stub {
        chat_mode,
        chat_requests: AtomicU32::new(0),
        moves_requests: AtomicU32::new(0),
        parents: Mutex::new(Vec::new()),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/game/{game}/{stream}", get(handler))
        .with_state(stub.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });
    Ok((url, stub))
}

fn fast() -> SupervisorConfig {
    SupervisorConfig {
        min_backoff: Duration::from_millis(40),
        max_backoff: Duration::from_millis(80),
        queue: 2,
    }
}

fn start(url: &str, resource: ChessResource, cursor: StreamCursor) -> StreamHandle {
    StreamHandle::spawn(
        url.to_owned(),
        "g1".into(),
        resource,
        cursor,
        fast(),
        &tokio::runtime::Handle::current(),
    )
}

async fn wait_for(mut condition: impl FnMut() -> bool) -> anyhow::Result<()> {
    for _ in 0..200 {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    anyhow::bail!("condition was not reached in time")
}

async fn next(handle: &mut StreamHandle) -> anyhow::Result<ChessMessage> {
    let delivery = tokio::time::timeout(Duration::from_secs(5), handle.recv())
        .await
        .context("timed out waiting for a delivery")?
        .context("stream stopped")?;
    let message = delivery.message.clone();
    delivery.receipt.acknowledge()?;
    Ok(message)
}

#[tokio::test]
async fn chat_refusal_leaves_moves_operational() -> anyhow::Result<()> {
    let (url, stub) = serve(ChatMode::Refuse).await?;
    let mut moves = start(&url, ChessResource::moves("g1"), StreamCursor::default());
    let chat = start(&url, ChessResource::chat("g1"), StreamCursor::default());

    assert!(matches!(next(&mut moves).await?, ChessMessage::Move(_)));
    wait_for(|| stub.chat_requests.load(Ordering::SeqCst) >= 2).await?;
    assert!(moves.is_online(), "moves must stay online");
    assert_ne!(chat.state(), StreamState::Online);
    assert_eq!(stub.moves_requests.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn established_chat_stream_closing_leaves_moves_operational() -> anyhow::Result<()> {
    let (url, stub) = serve(ChatMode::CloseAfterOne).await?;
    let mut moves = start(&url, ChessResource::moves("g1"), StreamCursor::default());
    let mut chat = start(&url, ChessResource::chat("g1"), StreamCursor::default());

    assert!(matches!(next(&mut chat).await?, ChessMessage::Chat(_)));
    // The server closed chat; the supervisor reconnects it on its own.
    wait_for(|| stub.chat_requests.load(Ordering::SeqCst) >= 2).await?;
    assert!(matches!(next(&mut moves).await?, ChessMessage::Move(_)));
    assert!(moves.is_online());
    assert_eq!(
        stub.moves_requests.load(Ordering::SeqCst),
        1,
        "moves must not reconnect because chat did"
    );
    Ok(())
}

#[tokio::test]
async fn expired_chat_history_clears_only_the_chat_cursor() -> anyhow::Result<()> {
    let (url, stub) = serve(ChatMode::GoneOnce).await?;
    let moves_cursor = StreamCursor::default();
    moves_cursor.set(vec![Version::new("5")]);
    let chat_cursor = StreamCursor::default();
    chat_cursor.set(vec![Version::new("7")]);
    let mut moves = start(&url, ChessResource::moves("g1"), moves_cursor.clone());
    let mut chat = start(&url, ChessResource::chat("g1"), chat_cursor.clone());

    assert!(matches!(next(&mut chat).await?, ChessMessage::Chat(_)));
    assert!(matches!(next(&mut moves).await?, ChessMessage::Move(_)));
    assert!(
        chat_cursor.get().is_empty(),
        "410 resets the affected stream"
    );
    assert_eq!(
        moves_cursor.get(),
        [Version::new("5")],
        "the sibling stream keeps its acknowledged frontier"
    );
    let seen = stub
        .parents
        .lock()
        .map_err(|_| anyhow::anyhow!("poisoned"))?;
    let chat_parents: Vec<_> = seen.iter().filter(|(s, _)| s == "chat").collect();
    assert_eq!(chat_parents[0].1.as_deref(), Some("\"7\""));
    assert_eq!(
        chat_parents[1].1, None,
        "replay after 410 starts from scratch"
    );
    let moves_parents: Vec<_> = seen.iter().filter(|(s, _)| s == "moves").collect();
    assert_eq!(moves_parents.len(), 1);
    assert_eq!(moves_parents[0].1.as_deref(), Some("\"5\""));
    Ok(())
}

#[tokio::test]
async fn dropping_the_handle_stops_the_task() -> anyhow::Result<()> {
    let (url, stub) = serve(ChatMode::Refuse).await?;
    let chat = start(&url, ChessResource::chat("g1"), StreamCursor::default());
    wait_for(|| stub.chat_requests.load(Ordering::SeqCst) >= 1).await?;
    drop(chat);
    let settled = stub.chat_requests.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        stub.chat_requests.load(Ordering::SeqCst) <= settled + 1,
        "no retry loop may survive the handle"
    );
    Ok(())
}
