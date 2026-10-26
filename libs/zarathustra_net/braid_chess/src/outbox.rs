//! Durable, serial publication of chess events through [`ReliableChannel`].
//!
//! One channel per stream owns the outbox: an event is committed to its journal
//! before any network side effect, sent strictly in order, retried with backoff,
//! and removed only after the application acknowledges the backend's acceptance
//! (`commit_ack`). The channel speaks the Braid PUT model; [`BackendNetwork`]
//! translates that to the game-log backend's JSON event contract (session
//! token, device id, optimistic parent) without ever changing a logical event:
//!
//! * identity, payload and version of a queued event are immutable;
//! * a `409 expected_parent` only changes the *transport attempt's* parent, and
//!   the backend accepts that attempt only if its own head matches, after first
//!   recognising an already-committed version (so a lost acknowledgement never
//!   creates a second event);
//! * `seat_superseded`, authorization failures and other 4xx stop the channel in
//!   `RepairRequired` with the outbox intact, and [`ChessOutbox::resume`] starts
//!   it again once the session has fresh credentials.
//!
//! Durable states of an event: *queued* (in the outbox journal, possibly
//! ambiguous after a send), *backend-confirmed* (`PutAcknowledged` observed, then
//! removed by `commit_ack`). Engine application and on-chain finality are the
//! application's own concerns and are never inferred from a network receipt.

use crate::{
    journal::{Journal, PendingWrite},
    resource::{ChessResource, ChessStream},
    ChessMessage,
};
use async_trait::async_trait;
use braid_http::{
    client::{ChannelConfig, ChannelEvent, ChannelJournal, ChannelState, ReliableChannel},
    traits::BraidNetwork,
    types::{BraidRequest, BraidResponse, Update, Version},
    BraidError,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError, RwLock},
    time::Duration,
};
use tokio::{runtime::Handle, task::JoinHandle};
use tracing::{debug, info, warn};

const SENDER_HEADER: &str = "x-xfchess-sender";
/// Parent corrections attempted inside one transport attempt before backing off.
const MAX_REPARENTS: usize = 8;

/// Request body of the backend's game-event endpoint.
#[derive(Serialize)]
struct GameEventRequest<'a> {
    #[serde(rename = "player_pubkey")]
    sender_identity: &'a str,
    session_token: &'a str,
    message: &'a ChessMessage,
    content_version: &'a str,
    content_parent: &'a str,
    device_id: &'a str,
}

#[derive(Deserialize)]
struct ParentMismatch {
    expected_parent: String,
}

/// Called with the stream and version the backend accepted.
pub type AcceptedHook = Arc<dyn Fn(ChessStream, &str) + Send + Sync>;

/// Application-supplied behaviour the transport cannot know.
#[derive(Clone)]
pub struct OutboxHooks {
    /// Authentication headers for the current login, read for every attempt.
    pub headers: Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>,
    /// Whether a 409 body means another device now holds this seat.
    pub is_seat_superseded: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// Called once when the seat was taken over.
    pub on_seat_superseded: Arc<dyn Fn() + Send + Sync>,
    /// Called when the backend accepted `version` on `stream`.
    pub on_accepted: AcceptedHook,
}

/// Where and for whom the outbox publishes.
#[derive(Clone)]
pub struct OutboxConfig {
    pub base_url: String,
    pub game_id: String,
    pub device_id: String,
    /// Per-game, per-device directory; channel journals live next to the legacy one.
    pub directory: PathBuf,
    pub put_timeout: Duration,
    pub max_pending: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("outbox storage: {0}")]
    Storage(String),
    #[error("outbox is full or the event was refused: {0}")]
    Refused(String),
}

#[derive(Clone, Default)]
struct Credentials {
    session_token: String,
}

struct BackendNetwork {
    client: reqwest::Client,
    device_id: String,
    credentials: Arc<RwLock<Credentials>>,
    hooks: OutboxHooks,
    /// Parents learned from the backend for versions still in flight.
    learned_parents: Mutex<HashMap<String, String>>,
}

impl BackendNetwork {
    fn token(&self) -> String {
        self.credentials
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .session_token
            .clone()
    }
}

fn first(versions: &Option<Vec<Version>>, what: &str) -> braid_http::Result<String> {
    versions
        .as_ref()
        .and_then(|versions| versions.first())
        .map(ToString::to_string)
        .ok_or_else(|| BraidError::Protocol(format!("queued event has no {what}")))
}

#[async_trait]
impl BraidNetwork for BackendNetwork {
    async fn fetch(&self, url: &str, request: BraidRequest) -> braid_http::Result<BraidResponse> {
        let version = first(&request.version, "version")?;
        let declared_parent = first(&request.parents, "parent")?;
        let sender = request
            .extra_headers
            .get(SENDER_HEADER)
            .cloned()
            .ok_or_else(|| BraidError::Protocol("queued event has no sender".into()))?;
        let message: ChessMessage = serde_json::from_slice(&request.body)?;
        let mut parent = self
            .learned_parents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&version)
            .cloned()
            .unwrap_or(declared_parent);
        for _ in 0..MAX_REPARENTS {
            let token = self.token();
            let body = GameEventRequest {
                sender_identity: &sender,
                session_token: &token,
                message: &message,
                content_version: &version,
                content_parent: &parent,
                device_id: &self.device_id,
            };
            let mut attempt = self.client.put(url).json(&body);
            for (name, value) in (self.hooks.headers)() {
                attempt = attempt.header(name, value);
            }
            let response = attempt
                .send()
                .await
                .map_err(|error| BraidError::Http(error.to_string()))?;
            let status = response.status();
            if status == reqwest::StatusCode::OK {
                self.learned_parents
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&version);
                return Ok(BraidResponse::new(200, Bytes::new()));
            }
            let text = response.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::CONFLICT {
                if (self.hooks.is_seat_superseded)(&text) {
                    (self.hooks.on_seat_superseded)();
                    return Err(BraidError::Config("seat superseded".into()));
                }
                let mismatch: ParentMismatch = serde_json::from_str(&text).map_err(|error| {
                    BraidError::Protocol(format!("unrecognised conflict: {error}"))
                })?;
                if mismatch.expected_parent == parent {
                    return Err(BraidError::Protocol(
                        "backend rejected an event whose parent it expects".into(),
                    ));
                }
                debug!(
                    "[outbox] backend head moved; retrying {version} on parent {}",
                    mismatch.expected_parent
                );
                parent = mismatch.expected_parent;
                self.learned_parents
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(version.clone(), parent.clone());
                continue;
            }
            // 5xx and 429 are retried by the channel; other statuses stop it.
            return Ok(BraidResponse::new(status.as_u16(), Bytes::from(text)));
        }
        // The head kept moving; back off and try again rather than fail the event.
        Ok(BraidResponse::new(503, Bytes::new()))
    }

    async fn subscribe(
        &self,
        _url: &str,
        _request: BraidRequest,
    ) -> braid_http::Result<async_channel::Receiver<braid_http::Result<Update>>> {
        Err(BraidError::CapabilityUnavailable {
            capability: "subscriptions on the publish channel",
        })
    }
}

struct StreamOutbox {
    channel: Arc<ReliableChannel>,
    pump: JoinHandle<()>,
}

impl Drop for StreamOutbox {
    fn drop(&mut self) {
        self.pump.abort();
    }
}

/// Serial, durable publisher for one game on one device.
pub struct ChessOutbox {
    config: OutboxConfig,
    hooks: OutboxHooks,
    runtime: Handle,
    network: Arc<BackendNetwork>,
    legacy: Option<Journal>,
    streams: tokio::sync::Mutex<HashMap<ChessStream, StreamOutbox>>,
}

fn stream_name(stream: ChessStream) -> &'static str {
    stream.as_str()
}

impl ChessOutbox {
    /// `legacy` is the pre-channel journal; its pending writes are moved into the
    /// channel outbox the first time a stream is opened.
    pub fn new(
        config: OutboxConfig,
        hooks: OutboxHooks,
        runtime: Handle,
        legacy: Option<Journal>,
    ) -> Result<Self, OutboxError> {
        let client = reqwest::Client::builder()
            .timeout(config.put_timeout)
            .build()
            .map_err(|error| OutboxError::Storage(error.to_string()))?;
        let network = Arc::new(BackendNetwork {
            client,
            device_id: config.device_id.clone(),
            credentials: Arc::default(),
            hooks: hooks.clone(),
            learned_parents: Mutex::default(),
        });
        Ok(Self {
            config,
            hooks,
            runtime,
            network,
            legacy,
            streams: tokio::sync::Mutex::default(),
        })
    }

    fn url(&self, stream: ChessStream) -> String {
        let resource = match stream {
            ChessStream::Chat => ChessResource::chat(&self.config.game_id),
            ChessStream::Clock => ChessResource::clock(&self.config.game_id),
            ChessStream::Engine => ChessResource::engine(&self.config.game_id),
            ChessStream::Moves => ChessResource::moves(&self.config.game_id),
        };
        resource.to_url(&self.config.base_url)
    }

    fn journal_path(&self, stream: ChessStream) -> PathBuf {
        self.config
            .directory
            .join(format!("outbox.{}.json", stream_name(stream)))
    }

    /// Replace the session credentials used by every later attempt.
    pub fn set_session_token(&self, session_token: &str) {
        // An empty token means "not available yet"; it must not erase a working one.
        if session_token.is_empty() {
            return;
        }
        self.network
            .credentials
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .session_token = session_token.to_owned();
    }

    async fn open_stream(&self, stream: ChessStream) -> Result<StreamOutbox, OutboxError> {
        let path = self.journal_path(stream);
        // A just-stopped channel releases its writer lease asynchronously.
        let mut journal = None;
        let mut last_error = String::new();
        for _ in 0..20 {
            match ChannelJournal::open(&path).await {
                Ok(opened) => {
                    journal = Some(opened);
                    break;
                }
                Err(error) => {
                    last_error = error.to_string();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
        let journal = journal.ok_or(OutboxError::Storage(last_error))?;
        let config = ChannelConfig {
            max_pending: self.config.max_pending,
            put_timeout: self.config.put_timeout,
            application_acknowledgments: true,
            ..ChannelConfig::default()
        };
        let channel = ReliableChannel::start_sender(
            self.network.clone(),
            self.url(stream),
            BraidRequest::new(),
            journal,
            config,
        )
        .map_err(|error| OutboxError::Storage(error.to_string()))?;
        let channel = Arc::new(channel);
        let pump = tokio::spawn(pump(channel.clone(), stream, self.hooks.clone()));
        let opened = StreamOutbox { channel, pump };
        self.migrate_legacy(stream, &opened).await?;
        Ok(opened)
    }

    /// Move writes left in the pre-channel journal into the channel outbox. The
    /// channel journal commits first; the legacy entry is removed only after, so a
    /// crash in between leaves the event in both places and the (idempotent)
    /// migration simply runs again.
    async fn migrate_legacy(
        &self,
        stream: ChessStream,
        opened: &StreamOutbox,
    ) -> Result<(), OutboxError> {
        let Some(legacy) = &self.legacy else {
            return Ok(());
        };
        let pending = legacy
            .pending()
            .map_err(|error| OutboxError::Storage(error.to_string()))?;
        for write in pending
            .into_iter()
            .filter(|write| write.stream == stream_name(stream))
        {
            let version = write.version.clone();
            let already_queued = opened
                .channel
                .pending_updates()
                .await
                .map(|queued| {
                    queued
                        .iter()
                        .any(|update| update.version == vec![Version::new(&version)])
                })
                .unwrap_or(false);
            if !already_queued {
                opened
                    .channel
                    .queue(update_for(&write)?, None)
                    .await
                    .map_err(|error| OutboxError::Refused(error.to_string()))?;
            }
            legacy
                .confirm_write(&version)
                .map_err(|error| OutboxError::Storage(error.to_string()))?;
            info!(
                "[outbox] moved pending {} write {version} into the channel",
                write.stream
            );
        }
        Ok(())
    }

    async fn stream_outbox(
        &self,
        stream: ChessStream,
    ) -> Result<Arc<ReliableChannel>, OutboxError> {
        let mut streams = self.streams.lock().await;
        let stopped = streams.get(&stream).is_some_and(|open| {
            matches!(
                open.channel.status().borrow().state,
                ChannelState::RepairRequired | ChannelState::Stopped
            )
        });
        if stopped {
            streams.remove(&stream);
        }
        if let Some(open) = streams.get(&stream) {
            return Ok(open.channel.clone());
        }
        let opened = self.open_stream(stream).await?;
        let channel = opened.channel.clone();
        streams.insert(stream, opened);
        Ok(channel)
    }

    /// Persist `write` and let the channel deliver it. Returns once the event is
    /// durable; delivery continues in the background.
    pub async fn publish_async(&self, write: PendingWrite) -> Result<(), OutboxError> {
        let stream = if write.stream == "chat" {
            ChessStream::Chat
        } else {
            ChessStream::Moves
        };
        let channel = self.stream_outbox(stream).await?;
        channel
            .queue(update_for(&write)?, None)
            .await
            .map_err(|error| OutboxError::Refused(error.to_string()))
    }

    /// Blocking form for callers outside the runtime (the game's systems).
    pub fn publish(&self, write: PendingWrite) -> Result<(), OutboxError> {
        self.runtime.block_on(self.publish_async(write))
    }

    /// Install fresh credentials and restart delivery on every stream that has
    /// work or had stopped, including streams whose outbox only exists on disk.
    pub async fn resume_async(&self, session_token: &str) {
        self.set_session_token(session_token);
        for stream in [ChessStream::Moves, ChessStream::Chat] {
            let has_disk_state = self.journal_path(stream).exists()
                || self.legacy.as_ref().is_some_and(|legacy| {
                    legacy
                        .pending()
                        .map(|pending| pending.iter().any(|w| w.stream == stream_name(stream)))
                        .unwrap_or(false)
                });
            if !has_disk_state {
                continue;
            }
            if let Err(error) = self.stream_outbox(stream).await {
                warn!("[outbox] could not resume {}: {error}", stream_name(stream));
            }
        }
    }

    pub fn resume(&self, session_token: &str) {
        self.runtime.block_on(self.resume_async(session_token));
    }

    /// Current state of a stream's channel, if it has been opened.
    pub async fn state(&self, stream: ChessStream) -> Option<ChannelState> {
        let streams = self.streams.lock().await;
        let open = streams.get(&stream)?;
        let state = open.channel.status().borrow().state;
        Some(state)
    }

    /// Events accepted into the outbox and not yet confirmed by the backend.
    pub async fn pending(&self, stream: ChessStream) -> usize {
        let streams = self.streams.lock().await;
        streams
            .get(&stream)
            .map_or(0, |open| open.channel.status().borrow().outstanding_puts)
    }
}

fn update_for(write: &PendingWrite) -> Result<Update, OutboxError> {
    let body = serde_json::to_vec(&write.message)
        .map_err(|error| OutboxError::Refused(error.to_string()))?;
    Ok(
        Update::snapshot(Version::new(&write.version), Bytes::from(body))
            .with_parents(vec![Version::new(&write.parent)])
            .with_content_type("application/json")
            .with_header(SENDER_HEADER, write.sender.clone()),
    )
}

/// Consumes channel events: records accepted events, then removes them from the
/// outbox. Ends when the channel stops.
async fn pump(channel: Arc<ReliableChannel>, stream: ChessStream, hooks: OutboxHooks) {
    while let Some(event) = channel.next_event().await {
        match event {
            Ok(ChannelEvent::PutAcknowledged { version, .. }) => {
                let accepted = version.first().map(ToString::to_string).unwrap_or_default();
                (hooks.on_accepted)(stream, &accepted);
                let mut attempts = 0;
                while let Err(error) = channel.commit_ack(version.clone(), None).await {
                    attempts += 1;
                    if attempts >= 5 {
                        warn!(
                            "[outbox] could not retire {accepted} from the {} outbox: {error}",
                            stream_name(stream)
                        );
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(200 * attempts)).await;
                }
                debug!(
                    "[outbox] {} event {accepted} confirmed",
                    stream_name(stream)
                );
            }
            Ok(ChannelEvent::Update(_)) => {}
            Err(error) => {
                warn!(
                    "[outbox] {} publishing stopped and needs a fresh session: {error}",
                    stream_name(stream)
                );
                return;
            }
        }
    }
}
