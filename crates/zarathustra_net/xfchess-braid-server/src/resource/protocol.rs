//! The server's update type, and its bridge onto the Braid wire.
//!
//! [`BraidUpdate`] is the *domain* type: a monotonic `u64` version, its parents,
//! and a JSON body. Turning it into bytes is not this module's job — that belongs
//! to [`braid_http::protocol::formatter`], the single emitter shared with the
//! client's parser.
//!
//! # Why the framing moved
//!
//! This module used to emit its own `multipart/mixed` framing with a
//! `--xfchess-braid` boundary. That is not Braid: the protocol has no multipart
//! boundaries. It went unnoticed because this workspace's own client skips header
//! lines containing no colon, so it silently discarded the boundary markers and
//! parsed the rest. The cost was that no braid.org tool — `view.braid.org`, the
//! browser extension, the JavaScript client — could read our streams; and the
//! old heartbeat (a complete empty *message* rather than a bare CRLF) decoded
//! into a versionless update that logged two warnings every 20 seconds per
//! subscriber.

use braid_http::protocol::formatter;
use braid_http::types::{Update, Version as WireVersion};
use serde::{Deserialize, Serialize};

/// A monotonic sequence counter used as the resource version.
pub type Version = u64;

/// A single streamed update — either an initial snapshot or a delta patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraidUpdate {
    /// This update's version.
    pub version: Version,
    /// Parent versions this update causally follows.
    pub parents: Vec<Version>,
    /// JSON body.  Full document for snapshots; JSON-Patch array for deltas.
    pub body: serde_json::Value,
    /// `true` when `body` is a full snapshot, `false` when it is a JSON-Patch.
    pub is_snapshot: bool,
}

impl BraidUpdate {
    pub fn snapshot(version: Version, body: serde_json::Value) -> Self {
        Self {
            version,
            parents: Vec::new(),
            body,
            is_snapshot: true,
        }
    }

    pub fn patch(version: Version, parent: Version, patches: serde_json::Value) -> Self {
        Self {
            version,
            parents: vec![parent],
            body: patches,
            is_snapshot: false,
        }
    }
}

impl From<&BraidUpdate> for Update {
    /// Map the server's `u64` version domain onto the protocol's opaque
    /// [`WireVersion`].
    ///
    /// Both snapshots and JSON-Patch deltas travel as a JSON *body*, not as Braid
    /// `Patches`. A Braid patch carries a `Content-Range` addressing a region of
    /// the resource; an RFC 6902 patch document is a self-contained JSON value
    /// that happens to describe a change. Sending it as a body keeps every chunk
    /// decodable by a client that reads bodies as JSON — which is exactly what
    /// `braid_chess::ChessSubscriber` does.
    fn from(update: &BraidUpdate) -> Self {
        let body = serde_json::to_string(&update.body).unwrap_or_else(|_| "null".to_string());

        // Both shapes travel as a body, so the media type is the only thing
        // that says which one this is: a full document, or the RFC 6902 patch
        // document that transforms the previous version into this one. A
        // receiver that ignores it and parses every body as the resource will
        // silently mistake a patch for the state.
        let content_type = if update.is_snapshot {
            "application/json"
        } else {
            "application/json-patch+json"
        };

        Update::snapshot(WireVersion::new(update.version.to_string()), body)
            .with_parents(
                update
                    .parents
                    .iter()
                    .map(|p| WireVersion::new(p.to_string()))
                    .collect(),
            )
            .with_content_type(content_type)
    }
}

/// Serialize one update onto the wire.
///
/// See [`formatter`](braid_http::protocol::formatter) for the exact byte layout.
pub fn format_chunk(update: &BraidUpdate) -> bytes::Bytes {
    formatter::format_update(&Update::from(update)).unwrap_or_else(|e| {
        // `format_update` only fails while serializing patches, and this
        // conversion never produces any — but never poison a live stream over it.
        tracing::error!("[braid] could not format update v{}: {e}", update.version);
        bytes::Bytes::new()
    })
}

/// The keep-alive bytes for an idle subscription: a bare CRLF, which a
/// conformant parser absorbs without producing an update.
pub fn format_heartbeat() -> bytes::Bytes {
    formatter::format_heartbeat()
}

/// Encode one update for a gossip transport, tagged with its resource path.
///
/// # Why this is not [`format_chunk`]
///
/// The `209` framing that `format_chunk` emits carries no resource identity —
/// over HTTP the request line already said which resource this is. Gossip has
/// no request: a peer receives bytes on a topic that may carry updates for
/// several resources, so the path has to travel *with* the update. [`Update`]
/// has a `url` field for exactly this, and it survives a JSON round-trip,
/// which the header framing does not.
///
/// This is the same convention `braid_iroh::SubscriptionManager::broadcast`
/// already uses (a JSON-serialized [`Update`]) — the only addition is that
/// `url` is populated, so a receiver can tell standings from pairings.
pub fn encode_for_gossip(path: &str, update: &BraidUpdate) -> Option<Vec<u8>> {
    let mut wire = Update::from(update);
    wire.url = Some(path.to_string());
    serde_json::to_vec(&wire)
        .inspect_err(|e| {
            tracing::error!(
                "[braid] could not encode update v{} for gossip: {e}",
                update.version
            )
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_http::client::MessageParser;
    use serde_json::json;

    #[test]
    fn a_snapshot_round_trips_through_the_wire() {
        let update = BraidUpdate::snapshot(7, json!({"fen": "startpos"}));
        let bytes = format_chunk(&update);

        let mut parser = MessageParser::for_subscription();
        let messages = parser.feed(&bytes).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].headers.get("version").unwrap(), "\"7\"");
        let body = messages[0].body.as_ref().unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(body).unwrap(),
            json!({"fen": "startpos"})
        );
    }

    #[test]
    fn a_patch_update_carries_its_parent() {
        let update = BraidUpdate::patch(8, 7, json!([{"op": "add", "path": "/-", "value": 1}]));
        let bytes = format_chunk(&update);

        let mut parser = MessageParser::for_subscription();
        let messages = parser.feed(&bytes).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].headers.get("parents").unwrap(), "\"7\"");
    }

    #[test]
    fn successive_updates_parse_as_separate_messages() {
        // The framing must be self-delimiting: this is what a subscriber actually
        // receives once several updates land in one TCP read.
        let mut wire = Vec::new();
        for v in 1..=3 {
            wire.extend_from_slice(&format_chunk(&BraidUpdate::snapshot(v, json!({"n": v}))));
        }

        let mut parser = MessageParser::for_subscription();
        let messages = parser.feed(&wire).unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn heartbeats_between_updates_produce_no_messages() {
        // The defect this replaced: heartbeats decoding as versionless updates,
        // which logged a warning per beat and reached subscribers as empty bodies.
        let mut wire = Vec::new();
        wire.extend_from_slice(&format_heartbeat());
        wire.extend_from_slice(&format_chunk(&BraidUpdate::snapshot(1, json!("a"))));
        wire.extend_from_slice(&format_heartbeat());
        wire.extend_from_slice(&format_heartbeat());
        wire.extend_from_slice(&format_chunk(&BraidUpdate::snapshot(2, json!("b"))));

        let mut parser = MessageParser::for_subscription();
        let messages = parser.feed(&wire).unwrap();

        assert_eq!(messages.len(), 2, "heartbeats must not become updates");
        for msg in &messages {
            assert!(
                msg.headers.contains_key("version"),
                "every delivered message must carry a Version"
            );
        }
    }

    #[test]
    fn the_wire_contains_no_multipart_boundary() {
        let bytes = format_chunk(&BraidUpdate::snapshot(1, json!("x")));
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("--"), "multipart boundary leaked: {text:?}");
        assert!(!text.contains("multipart"));
    }

    #[test]
    fn a_stream_split_across_reads_still_parses() {
        // TCP does not respect message boundaries; the parser must not either.
        let bytes = format_chunk(&BraidUpdate::snapshot(42, json!({"a": "b"})));
        let mut parser = MessageParser::for_subscription();

        let mut total = Vec::new();
        for byte in bytes.iter() {
            total.extend(parser.feed(&[*byte]).unwrap());
        }
        assert_eq!(total.len(), 1);
        assert_eq!(total[0].headers.get("version").unwrap(), "\"42\"");
    }
}
