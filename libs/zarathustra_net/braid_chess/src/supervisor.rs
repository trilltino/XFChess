//! Supervised, independent subscriptions for chess resources.
//!
//! Every [`ChessResource`] gets its own task, cancellation, resume cursor,
//! bounded delivery queue and connection state. A chat outage, a 410 on one
//! stream, or a slow consumer of one stream can therefore never interrupt
//! another stream's delivery or discard its acknowledged frontier.

use crate::{
    error::BraidChessError,
    resource::ChessResource,
    subscriber::{ChessDelivery, ChessSubscriber},
};
use braid_http::Version;
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex, PoisonError,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{runtime::Handle, sync::mpsc, task::JoinHandle};
use tracing::{debug, info, warn};

/// Lifecycle of one supervised subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// A connection attempt is in flight.
    Connecting,
    /// The server accepted the subscription and the stream is open.
    Online,
    /// The last attempt failed or the stream ended; a retry is scheduled.
    Backoff,
    /// Retrying cannot help (authorization or protocol failure).
    RepairRequired,
    /// The task has ended because its consumer went away or it was dropped.
    Stopped,
}

impl StreamState {
    fn to_raw(self) -> u8 {
        match self {
            Self::Connecting => 0,
            Self::Online => 1,
            Self::Backoff => 2,
            Self::RepairRequired => 3,
            Self::Stopped => 4,
        }
    }

    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Connecting,
            1 => Self::Online,
            2 => Self::Backoff,
            3 => Self::RepairRequired,
            _ => Self::Stopped,
        }
    }
}

/// The last application-acknowledged frontier of one stream.
///
/// Each stream owns its own cursor, so clearing one can never clear another.
#[derive(Debug, Clone, Default)]
pub struct StreamCursor(Arc<Mutex<Vec<Version>>>);

impl StreamCursor {
    pub fn get(&self) -> Vec<Version> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set(&self, versions: Vec<Version>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = versions;
    }

    pub fn clear(&self) {
        self.set(Vec::new());
    }
}

/// How the supervisor reacts to a failed subscription attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// Transient failure: retry with backoff, keeping the cursor.
    Retry,
    /// The server no longer retains the cursor: drop it, then replay.
    ResetCursor,
    /// Retrying cannot succeed without outside intervention.
    RepairRequired,
}

/// Classify a subscription failure into a recovery action.
pub fn classify(error: &BraidChessError) -> Recovery {
    match error {
        BraidChessError::HttpStatus(410) => Recovery::ResetCursor,
        BraidChessError::HttpStatus(400 | 401 | 403 | 405 | 406 | 422) => Recovery::RepairRequired,
        _ => Recovery::Retry,
    }
}

/// Timing and queue bounds for a supervised stream.
#[derive(Debug, Clone, Copy)]
pub struct SupervisorConfig {
    pub min_backoff: Duration,
    pub max_backoff: Duration,
    /// Deliveries buffered ahead of the consumer before the stream backpressures.
    pub queue: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            queue: 2,
        }
    }
}

/// Owner of one supervised subscription. Dropping it cancels the task.
pub struct StreamHandle {
    resource: ChessResource,
    deliveries: mpsc::Receiver<ChessDelivery>,
    state: Arc<AtomicU8>,
    cursor: StreamCursor,
    task: JoinHandle<()>,
}

impl StreamHandle {
    /// Start supervising `resource` on `runtime`, resuming from `cursor`.
    pub fn spawn(
        base_url: String,
        game_id: String,
        resource: ChessResource,
        cursor: StreamCursor,
        config: SupervisorConfig,
        runtime: &Handle,
    ) -> Self {
        let (tx, deliveries) = mpsc::channel(config.queue.max(1));
        let state = Arc::new(AtomicU8::new(StreamState::Connecting.to_raw()));
        let task = runtime.spawn(supervise(
            base_url,
            game_id,
            resource.clone(),
            tx,
            state.clone(),
            cursor.clone(),
            config,
        ));
        Self {
            resource,
            deliveries,
            state,
            cursor,
            task,
        }
    }

    pub fn resource(&self) -> &ChessResource {
        &self.resource
    }

    pub fn state(&self) -> StreamState {
        StreamState::from_raw(self.state.load(Ordering::Relaxed))
    }

    pub fn is_online(&self) -> bool {
        self.state() == StreamState::Online
    }

    pub fn cursor(&self) -> &StreamCursor {
        &self.cursor
    }

    /// Take the next buffered delivery without waiting.
    pub fn try_recv(&mut self) -> Option<ChessDelivery> {
        self.deliveries.try_recv().ok()
    }

    /// Wait for the next delivery; `None` once the supervisor has stopped.
    pub async fn recv(&mut self) -> Option<ChessDelivery> {
        self.deliveries.recv().await
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.task.abort();
        self.state
            .store(StreamState::Stopped.to_raw(), Ordering::Relaxed);
    }
}

fn jittered(base: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let quarter = u64::try_from(base.as_millis() / 4).unwrap_or(0).max(1);
    base + Duration::from_millis(u64::from(nanos) % quarter)
}

async fn supervise(
    base_url: String,
    game_id: String,
    resource: ChessResource,
    tx: mpsc::Sender<ChessDelivery>,
    state: Arc<AtomicU8>,
    cursor: StreamCursor,
    config: SupervisorConfig,
) {
    let set = |next: StreamState| state.store(next.to_raw(), Ordering::Relaxed);
    let name = resource.stream.as_str();
    let mut backoff = config.min_backoff;
    let mut recovering = false;
    loop {
        if tx.is_closed() {
            break;
        }
        set(StreamState::Connecting);
        let attempt = async {
            let subscriber = ChessSubscriber::new(&base_url, &game_id)?;
            subscriber
                .subscribe_acknowledged(resource.clone(), cursor.get())
                .await
        }
        .await;
        match attempt {
            Ok((rx, bridge)) => {
                set(StreamState::Online);
                backoff = config.min_backoff;
                if recovering {
                    info!("[braid] {name} stream reconnected for game {game_id}");
                } else {
                    debug!("[braid] subscribed to {name} for game {game_id}");
                }
                let consumer_gone = loop {
                    tokio::select! {
                        _ = tx.closed() => break true,
                        delivery = rx.recv() => match delivery {
                            Ok(delivery) => {
                                if tx.send(delivery).await.is_err() {
                                    break true;
                                }
                            }
                            Err(_) => break false,
                        },
                    }
                };
                bridge.abort();
                if consumer_gone {
                    break;
                }
                warn!("[braid] {name} stream lost for game {game_id}; reconnecting");
                set(StreamState::Backoff);
                recovering = true;
                tokio::time::sleep(jittered(config.min_backoff)).await;
            }
            Err(error) => {
                match classify(&error) {
                    Recovery::RepairRequired => {
                        warn!("[braid] {name} stream for game {game_id} needs repair: {error}");
                        set(StreamState::RepairRequired);
                        return;
                    }
                    recovery => {
                        if recovery == Recovery::ResetCursor {
                            // Only this stream replays; sibling streams keep their frontier.
                            cursor.clear();
                        }
                        warn!("[braid] {name} stream for game {game_id} unavailable: {error}; retrying");
                        set(StreamState::Backoff);
                        recovering = true;
                        tokio::time::sleep(jittered(backoff)).await;
                        backoff = (backoff * 2).min(config.max_backoff);
                    }
                }
            }
        }
    }
    set(StreamState::Stopped);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_expired_history_resets_the_cursor() {
        assert_eq!(
            classify(&BraidChessError::HttpStatus(410)),
            Recovery::ResetCursor
        );
        for status in [401, 403, 400] {
            assert_eq!(
                classify(&BraidChessError::HttpStatus(status)),
                Recovery::RepairRequired
            );
        }
        for status in [429, 500, 502, 503] {
            assert_eq!(
                classify(&BraidChessError::HttpStatus(status)),
                Recovery::Retry
            );
        }
        assert_eq!(
            classify(&BraidChessError::Http("connection refused".into())),
            Recovery::Retry
        );
    }

    #[test]
    fn cursors_are_independent() {
        let moves = StreamCursor::default();
        let chat = StreamCursor::default();
        moves.set(vec![Version::new("5")]);
        chat.set(vec![Version::new("7")]);
        chat.clear();
        assert_eq!(moves.get(), [Version::new("5")]);
        assert!(chat.get().is_empty());
    }

    #[test]
    fn jitter_stays_within_a_quarter_of_the_base() {
        let base = Duration::from_secs(4);
        for _ in 0..100 {
            let value = jittered(base);
            assert!(value >= base && value < base + Duration::from_secs(1));
        }
    }
}
