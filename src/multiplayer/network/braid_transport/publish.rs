//! Hand game events to the durable outbox.
//!
//! Ordering, retries, parent corrections and credential failures live in
//! [`braid_chess::ChessOutbox`] (one `ReliableChannel` per stream). This module
//! only decides an event's parent from the stream heads, records that the event
//! is our own so its echo is not applied twice, and persists it before returning.

use super::{open_journal, SharedStreamHeads};
use braid_chess::{journal::PendingWrite, ChessMessage, ChessOutbox};
use std::sync::Arc;
use tracing::warn;

fn outbox_of(heads: &SharedStreamHeads) -> Option<Arc<ChessOutbox>> {
    heads.lock().ok().and_then(|heads| heads.outbox.clone())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish(
    base_url: String,
    game_id: String,
    stream: &'static str,
    sender_identity: String,
    session_token: String,
    message: ChessMessage,
    content_version: String,
    heads: SharedStreamHeads,
) {
    let result = (|| {
        let parent = {
            let mut heads = heads
                .lock()
                .map_err(|_| std::io::Error::other("stream heads poisoned"))?;
            heads.self_published.insert(content_version.clone());
            heads.head(stream)
        };
        let write = PendingWrite {
            stream: stream.into(),
            sender: sender_identity,
            message,
            version: content_version,
            parent,
        };
        match outbox_of(&heads) {
            Some(outbox) => {
                outbox.set_session_token(&session_token);
                outbox.publish(write).map_err(std::io::Error::other)
            }
            // Before the subscription exists there is no outbox yet. The older
            // journal is durable too, and the outbox adopts its entries on resume.
            None => open_journal(&base_url, &game_id)?.enqueue(write),
        }
    })();
    if let Err(error) = result {
        warn!("[braid] could not save pending event for game {game_id}: {error}");
    }
}

/// Offer fresh credentials and restart delivery on streams that stopped or whose
/// outbox was left on disk by an earlier run.
pub(crate) fn retry_pending(
    _base_url: String,
    _game_id: String,
    _sender: String,
    session_token: String,
    heads: SharedStreamHeads,
) {
    let Some(outbox) = outbox_of(&heads) else {
        return;
    };
    crate::core::net_task::spawn_detached("publish-resume", move || outbox.resume(&session_token));
}
