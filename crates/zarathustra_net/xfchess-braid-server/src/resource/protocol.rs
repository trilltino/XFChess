//! Wire types for the Braid-HTTP 209 protocol.
//!
//! A Braid subscription streams a sequence of [`BraidUpdate`]s. Each update
//! carries a monotonically increasing [`Version`], a list of parent versions
//! it causally depends on, and either a full snapshot body or a JSON-Patch
//! delta body.
//!
//! Reference: <https://braid.org/meeting/90>

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
        Self { version, parents: Vec::new(), body, is_snapshot: true }
    }

    pub fn patch(version: Version, parent: Version, patches: serde_json::Value) -> Self {
        Self { version, parents: vec![parent], body: patches, is_snapshot: false }
    }
}

/// Format one Braid multipart chunk.
///
/// Wire format:
/// ```text
/// \r\n--<boundary>\r\n
/// Version: <n>\r\n
/// Parents: <p1>, <p2>\r\n          (omitted when empty)
/// Content-Length: <bytes>\r\n
/// Content-Type: application/json\r\n
/// \r\n
/// <body bytes>
/// ```
pub fn format_chunk(boundary: &str, update: &BraidUpdate) -> bytes::Bytes {
    let body = serde_json::to_string(&update.body).unwrap_or_default();
    let mut buf = Vec::with_capacity(256 + body.len());

    buf.extend_from_slice(b"\r\n--");
    buf.extend_from_slice(boundary.as_bytes());
    buf.extend_from_slice(b"\r\n");

    let ver_str = update.version.to_string();
    buf.extend_from_slice(b"Version: \"");
    buf.extend_from_slice(ver_str.as_bytes());
    buf.extend_from_slice(b"\"\r\n");

    if !update.parents.is_empty() {
        buf.extend_from_slice(b"Parents: ");
        let parents: Vec<String> = update.parents.iter().map(|p| format!("\"{}\"", p)).collect();
        buf.extend_from_slice(parents.join(", ").as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    buf.extend_from_slice(b"Content-Length: ");
    buf.extend_from_slice(body.len().to_string().as_bytes());
    buf.extend_from_slice(b"\r\nContent-Type: application/json\r\n\r\n");
    buf.extend_from_slice(body.as_bytes());

    bytes::Bytes::from(buf)
}

/// Heartbeat chunk — empty body, no version increment.
pub fn format_heartbeat(boundary: &str) -> bytes::Bytes {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(b"\r\n--");
    buf.extend_from_slice(boundary.as_bytes());
    buf.extend_from_slice(b"\r\nContent-Length: 0\r\n\r\n");
    bytes::Bytes::from(buf)
}
