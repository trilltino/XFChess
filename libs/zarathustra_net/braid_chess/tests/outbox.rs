//! The durable publish path against a stand-in for the game-log backend: events
//! are persisted first, sent in order, retried without duplicating, re-parented
//! only per attempt, and survive restarts and credential failures.

use anyhow::Context as _;
use axum::{extract::State, http::StatusCode, routing::put, Json, Router};
use braid_chess::{
    journal::{Journal, PendingWrite},
    ChessMessage, ChessOutbox, ChessStream, MovePayload, OutboxConfig, OutboxHooks,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Copy)]
enum Scripted {
    /// Reject without committing.
    Status(u16),
    /// Commit, then fail the response (a lost acknowledgement).
    CommitThenStatus(u16),
    SeatSuperseded,
}

struct Attempt {
    version: String,
    parent: String,
    token: String,
    device: String,
    sender: String,
}

#[derive(Default)]
struct Backend {
    head: Mutex<String>,
    committed: Mutex<Vec<(String, Value)>>,
    attempts: Mutex<Vec<Attempt>>,
    script: Mutex<VecDeque<Scripted>>,
    down: AtomicBool,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

async fn handle(
    State(backend): State<Arc<Backend>>,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    let text = |key: &str| body[key].as_str().unwrap_or_default().to_owned();
    let (version, parent) = (text("content_version"), text("content_parent"));
    locked(&backend.attempts).push(Attempt {
        version: version.clone(),
        parent: parent.clone(),
        token: text("session_token"),
        device: text("device_id"),
        sender: text("player_pubkey"),
    });
    if backend.down.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, String::new());
    }
    if text("session_token") != "good" {
        return (StatusCode::UNAUTHORIZED, String::new());
    }
    let scripted = locked(&backend.script).pop_front();
    if let Some(Scripted::SeatSuperseded) = scripted {
        return (
            StatusCode::CONFLICT,
            json!({"error": "seat_superseded"}).to_string(),
        );
    }
    if let Some(Scripted::Status(code)) = scripted {
        return (
            StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_GATEWAY),
            String::new(),
        );
    }
    // Like the real log, a version that is already committed is recognised before
    // the parent is checked.
    let already = locked(&backend.committed)
        .iter()
        .any(|(v, _)| *v == version);
    let mut head = locked(&backend.head);
    if !already {
        if parent != *head {
            return (
                StatusCode::CONFLICT,
                json!({"expected_parent": head.clone()}).to_string(),
            );
        }
        locked(&backend.committed).push((version.clone(), body["message"].clone()));
        *head = version;
    }
    if let Some(Scripted::CommitThenStatus(code)) = scripted {
        return (
            StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_GATEWAY),
            String::new(),
        );
    }
    (StatusCode::OK, String::new())
}

struct Harness {
    url: String,
    backend: Arc<Backend>,
    accepted: Arc<Mutex<Vec<String>>>,
    superseded: Arc<AtomicU32>,
    directory: tempfile::TempDir,
}

async fn harness() -> anyhow::Result<Harness> {
    let backend = Arc::new(Backend {
        head: Mutex::new("0".into()),
        ..Backend::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/game/{game}/{stream}", put(handle))
        .with_state(backend.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });
    Ok(Harness {
        url,
        backend,
        accepted: Arc::default(),
        superseded: Arc::default(),
        directory: tempfile::tempdir()?,
    })
}

impl Harness {
    fn outbox(&self, legacy: Option<Journal>) -> anyhow::Result<ChessOutbox> {
        let (accepted, superseded) = (self.accepted.clone(), self.superseded.clone());
        let hooks = OutboxHooks {
            headers: Arc::new(Vec::new),
            is_seat_superseded: Arc::new(|body| body.contains("seat_superseded")),
            on_seat_superseded: Arc::new(move || {
                superseded.fetch_add(1, Ordering::SeqCst);
            }),
            on_accepted: Arc::new(move |_stream, version| {
                locked(&accepted).push(version.to_owned());
            }),
        };
        let outbox = ChessOutbox::new(
            OutboxConfig {
                base_url: self.url.clone(),
                game_id: "g1".into(),
                device_id: "device-1".into(),
                directory: self.directory.path().join("g1"),
                put_timeout: Duration::from_secs(5),
                max_pending: 64,
            },
            hooks,
            tokio::runtime::Handle::current(),
            legacy,
        )?;
        outbox.set_session_token("good");
        Ok(outbox)
    }

    fn committed(&self) -> Vec<String> {
        locked(&self.backend.committed)
            .iter()
            .map(|(version, _)| version.clone())
            .collect()
    }
}

fn write(version: &str, parent: &str, fen: &str, number: u32) -> PendingWrite {
    PendingWrite {
        stream: "moves".into(),
        sender: "alice".into(),
        message: ChessMessage::Move(MovePayload::from_uci("e2e4", fen, number, "p")),
        version: version.into(),
        parent: parent.into(),
    }
}

async fn eventually(mut condition: impl FnMut() -> bool) -> anyhow::Result<()> {
    for _ in 0..400 {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    anyhow::bail!("condition was not reached in time")
}

#[tokio::test]
async fn events_are_sent_in_order_with_the_session_identity() -> anyhow::Result<()> {
    let harness = harness().await?;
    let outbox = harness.outbox(None)?;
    outbox.publish_async(write("v1", "0", "fen1", 1)).await?;
    outbox.publish_async(write("v2", "v1", "fen2", 2)).await?;
    outbox.publish_async(write("v3", "v2", "fen3", 3)).await?;

    eventually(|| locked(&harness.accepted).len() == 3).await?;
    assert_eq!(harness.committed(), ["v1", "v2", "v3"]);
    assert_eq!(*locked(&harness.accepted), ["v1", "v2", "v3"]);
    eventually(|| {
        matches!(
            futures::executor::block_on(outbox.pending(ChessStream::Moves)),
            0
        )
    })
    .await?;
    let attempts = locked(&harness.backend.attempts);
    assert!(attempts
        .iter()
        .all(|a| a.token == "good" && a.device == "device-1" && a.sender == "alice"));
    Ok(())
}

#[tokio::test]
async fn a_moved_head_re_parents_the_attempt_not_the_event() -> anyhow::Result<()> {
    let harness = harness().await?;
    // The opponent's event is already the head when ours (built on "0") is sent.
    *locked(&harness.backend.head) = "opponent-1".into();
    let outbox = harness.outbox(None)?;
    outbox.publish_async(write("v1", "0", "fen1", 1)).await?;

    eventually(|| harness.committed() == ["v1"]).await?;
    let attempts = locked(&harness.backend.attempts);
    let parents: Vec<_> = attempts.iter().map(|a| a.parent.as_str()).collect();
    assert_eq!(parents, ["0", "opponent-1"]);
    assert!(attempts.iter().all(|a| a.version == "v1"));
    assert_eq!(locked(&harness.backend.committed).len(), 1);
    Ok(())
}

#[tokio::test]
async fn a_lost_acknowledgement_never_creates_a_second_event() -> anyhow::Result<()> {
    let harness = harness().await?;
    locked(&harness.backend.script).push_back(Scripted::CommitThenStatus(500));
    let outbox = harness.outbox(None)?;
    outbox.publish_async(write("v1", "0", "fen1", 1)).await?;
    // The opponent moves before the retry, so the retry's parent is stale.
    eventually(|| harness.committed() == ["v1"]).await?;
    *locked(&harness.backend.head) = "opponent-2".into();

    eventually(|| locked(&harness.accepted).len() == 1).await?;
    assert_eq!(harness.committed(), ["v1"], "exactly one logical event");
    assert!(locked(&harness.backend.attempts).len() >= 2);
    Ok(())
}

#[tokio::test]
async fn pending_events_survive_a_restart_and_are_delivered_once() -> anyhow::Result<()> {
    let harness = harness().await?;
    harness.backend.down.store(true, Ordering::SeqCst);
    let first = harness.outbox(None)?;
    first.publish_async(write("v1", "0", "fen1", 1)).await?;
    first.publish_async(write("v2", "v1", "fen2", 2)).await?;
    eventually(|| !locked(&harness.backend.attempts).is_empty()).await?;
    drop(first);

    harness.backend.down.store(false, Ordering::SeqCst);
    let second = harness.outbox(None)?;
    second.resume_async("good").await;
    eventually(|| harness.committed().len() == 2).await?;
    assert_eq!(harness.committed(), ["v1", "v2"]);
    Ok(())
}

#[tokio::test]
async fn writes_left_in_the_legacy_journal_move_into_the_channel() -> anyhow::Result<()> {
    let harness = harness().await?;
    let legacy = Journal::open(harness.directory.path(), "legacy-scope")?;
    legacy.enqueue(write("v1", "0", "fen1", 1))?;
    legacy.enqueue(write("v2", "v1", "fen2", 2))?;

    let outbox = harness.outbox(Some(legacy.clone()))?;
    outbox.resume_async("good").await;
    eventually(|| harness.committed().len() == 2).await?;
    assert_eq!(harness.committed(), ["v1", "v2"]);
    assert!(
        legacy.pending()?.is_empty(),
        "nothing is left to send twice"
    );
    Ok(())
}

#[tokio::test]
async fn a_taken_seat_stops_the_stream_but_keeps_the_event() -> anyhow::Result<()> {
    let harness = harness().await?;
    locked(&harness.backend.script).push_back(Scripted::SeatSuperseded);
    let outbox = harness.outbox(None)?;
    outbox.publish_async(write("v1", "0", "fen1", 1)).await?;

    eventually(|| harness.superseded.load(Ordering::SeqCst) == 1).await?;
    eventually(|| {
        futures::executor::block_on(outbox.state(ChessStream::Moves))
            == Some(braid_http_state_repair())
    })
    .await?;
    assert!(harness.committed().is_empty());
    assert_eq!(outbox.pending(ChessStream::Moves).await, 1);

    // Reclaiming the seat resumes delivery of the same event.
    outbox.resume_async("good").await;
    eventually(|| harness.committed() == ["v1"]).await?;
    Ok(())
}

fn braid_http_state_repair() -> braid_chess::braid_http::client::ChannelState {
    braid_chess::braid_http::client::ChannelState::RepairRequired
}

#[tokio::test]
async fn an_expired_session_waits_for_fresh_credentials() -> anyhow::Result<()> {
    let harness = harness().await?;
    let outbox = harness.outbox(None)?;
    outbox.set_session_token("expired");
    outbox.publish_async(write("v1", "0", "fen1", 1)).await?;

    eventually(|| {
        futures::executor::block_on(outbox.state(ChessStream::Moves))
            == Some(braid_http_state_repair())
    })
    .await?;
    assert!(harness.committed().is_empty());
    assert_eq!(locked(&harness.backend.attempts)[0].token, "expired");

    outbox.resume_async("good").await;
    eventually(|| harness.committed() == ["v1"]).await?;
    let state = outbox
        .state(ChessStream::Moves)
        .await
        .context("stream open")?;
    assert_ne!(state, braid_http_state_repair());
    Ok(())
}
