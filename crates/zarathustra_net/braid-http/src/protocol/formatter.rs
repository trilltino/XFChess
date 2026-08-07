//! Serializes [`Update`]s onto the Braid-HTTP wire.
//!
//! This is the exact inverse of [`crate::client::parser::MessageParser`], and the
//! single place in the workspace that decides what a Braid update looks like in
//! bytes. Servers (`xfchess-braid-server`, the backend game log) emit through
//! here; clients parse through the parser. `tests/wire_roundtrip.rs` holds the
//! two against each other.
//!
//! # Wire format
//!
//! A `209` response body is a sequence of updates. Each update is a block of
//! headers, a blank line, then a payload:
//!
//! ```text
//! Version: "5"\r\n
//! Parents: "4"\r\n
//! Content-Type: application/json\r\n
//! Content-Length: 17\r\n
//! \r\n
//! {"move":"e2e4"}\r\n
//! ```
//!
//! A *patched* update replaces `Content-Length` with `Patches: N` and follows the
//! blank line with `N` patch blocks, each itself headers + blank line + content:
//!
//! ```text
//! Version: "6"\r\n
//! Patches: 1\r\n
//! \r\n
//! Content-Length: 4\r\n
//! Content-Range: json .clock\r\n
//! \r\n
//! 1234\r\n
//! ```
//!
//! `Patches: 0` is a legal update carrying no patches and **no body** — it ends at
//! the blank line. It is distinct from a snapshot with an empty body, and the
//! parser preserves that distinction.
//!
//! There are no MIME multipart boundaries anywhere in this protocol. Successive
//! updates are separated by the trailing `\r\n` after each payload, which the
//! parser skips on its way into the next header block.

use crate::error::Result;
use crate::protocol;
use crate::protocol::constants::headers;
use crate::types::{Patch, Update};
use bytes::{Bytes, BytesMut};

/// The bytes a server sends to keep an idle subscription alive.
///
/// A heartbeat is a bare CRLF. Because the parser skips leading newlines before
/// every header block, it decodes to *nothing* — no [`Update`] is produced and no
/// version is consumed. That is the whole point: a heartbeat proves the socket is
/// alive without being mistaken for a change to the resource.
#[must_use]
pub fn format_heartbeat() -> Bytes {
    Bytes::from_static(b"\r\n")
}

/// Serialize one [`Update`] into its wire bytes, including the trailing separator.
///
/// The result is self-delimiting: concatenating the output of successive calls
/// produces a valid `209` stream body.
///
/// # Errors
///
/// Returns an error if a patch cannot be serialized.
pub fn format_update(update: &Update) -> Result<Bytes> {
    let mut buffer = BytesMut::new();

    if !update.version.is_empty() {
        write_header(
            &mut buffer,
            headers::VERSION.as_str(),
            &protocol::format_version_header(&update.version),
        );
    }

    if !update.parents.is_empty() {
        write_header(
            &mut buffer,
            headers::PARENTS.as_str(),
            &protocol::format_version_header(&update.parents),
        );
    }

    if let Some(merge_type) = &update.merge_type {
        write_header(&mut buffer, headers::MERGE_TYPE.as_str(), merge_type);
    }

    if let Some(ct) = &update.content_type {
        write_header(&mut buffer, headers::CONTENT_TYPE.as_str(), ct);
    }

    for (k, v) in &update.extra_headers {
        write_header(&mut buffer, k, v);
    }

    match (&update.body, &update.patches) {
        // Patched update — `Patches: N` then N patch blocks. Checked before the
        // body arm so an update carrying both stays a patch update; `Update`'s
        // constructors never set both.
        (_, Some(patches)) => {
            write_header(
                &mut buffer,
                headers::PATCHES.as_str(),
                &patches.len().to_string(),
            );
            end_headers(&mut buffer);
            // `Patches: 0` ends here: no patch blocks, no body, no separator.
            for patch in patches {
                format_patch(&mut buffer, patch)?;
            }
        }
        // Snapshot — `Content-Length` then the body.
        (Some(body), None) => {
            write_header(
                &mut buffer,
                headers::CONTENT_LENGTH.as_str(),
                &body.len().to_string(),
            );
            end_headers(&mut buffer);
            buffer.extend_from_slice(body);
            end_payload(&mut buffer);
        }
        // Neither: an explicit empty snapshot.
        (None, None) => {
            write_header(&mut buffer, headers::CONTENT_LENGTH.as_str(), "0");
            end_headers(&mut buffer);
            end_payload(&mut buffer);
        }
    }

    Ok(buffer.freeze())
}

fn write_header(buffer: &mut BytesMut, key: &str, value: &str) {
    write_canonical_name(buffer, key);
    buffer.extend_from_slice(b": ");
    buffer.extend_from_slice(value.as_bytes());
    buffer.extend_from_slice(b"\r\n");
}

/// Write a header name in canonical `Title-Case` form.
///
/// [`http::HeaderName`] is lowercase by contract, but the reference implementation
/// emits `Version:` / `Content-Length:`, and that is what a human — or
/// `view.braid.org` — reads off the wire. HTTP header names are case-insensitive,
/// so this is presentation only; deriving it here keeps one list of header names
/// rather than two that can drift.
fn write_canonical_name(buffer: &mut BytesMut, key: &str) {
    let mut at_word_start = true;
    for byte in key.bytes() {
        if at_word_start {
            buffer.extend_from_slice(&[byte.to_ascii_uppercase()]);
        } else {
            buffer.extend_from_slice(&[byte]);
        }
        at_word_start = byte == b'-';
    }
}

/// The blank line closing a header block.
fn end_headers(buffer: &mut BytesMut) {
    buffer.extend_from_slice(b"\r\n");
}

/// The separator after a payload, which the parser skips before the next block.
fn end_payload(buffer: &mut BytesMut) {
    buffer.extend_from_slice(b"\r\n");
}

fn format_patch(buffer: &mut BytesMut, patch: &Patch) -> Result<()> {
    // Every patch MUST carry Content-Length — the parser rejects one that doesn't.
    write_header(
        buffer,
        headers::CONTENT_LENGTH.as_str(),
        &patch.content.len().to_string(),
    );
    write_header(
        buffer,
        headers::CONTENT_RANGE.as_str(),
        &format!("{} {}", patch.unit, patch.range),
    );
    end_headers(buffer);
    buffer.extend_from_slice(&patch.content);
    end_payload(buffer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Version;

    fn rendered(update: &Update) -> String {
        String::from_utf8(format_update(update).unwrap().to_vec()).unwrap()
    }

    #[test]
    fn snapshot_is_headers_then_body() {
        let s = rendered(&Update::snapshot(Version::new("v1"), "data"));
        assert_eq!(s, "Version: \"v1\"\r\nContent-Length: 4\r\n\r\ndata\r\n");
    }

    #[test]
    fn parents_are_emitted_before_the_payload() {
        let update = Update::snapshot(Version::new("v2"), "x").with_parent(Version::new("v1"));
        assert!(rendered(&update).starts_with("Version: \"v2\"\r\nParents: \"v1\"\r\n"));
    }

    #[test]
    fn no_multipart_boundary_appears_anywhere() {
        // Guards the defect this module was rewritten to fix: the old server
        // emitted `\r\n--boundary\r\n` delimiters, which are not part of Braid.
        let s = rendered(&Update::snapshot(Version::new("v1"), "data"));
        assert!(!s.contains("--"), "boundary delimiter leaked into the wire");
    }

    #[test]
    fn patched_update_emits_a_block_per_patch() {
        let update = Update::patched(
            Version::new("v1"),
            vec![Patch::json(".a", "1"), Patch::json(".b", "22")],
        );
        assert_eq!(
            rendered(&update),
            "Version: \"v1\"\r\nPatches: 2\r\n\r\n\
             Content-Length: 1\r\nContent-Range: json .a\r\n\r\n1\r\n\
             Content-Length: 2\r\nContent-Range: json .b\r\n\r\n22\r\n"
        );
    }

    #[test]
    fn zero_patches_ends_at_the_blank_line() {
        // `Patches: 0` carries no body at all — distinct from an empty snapshot.
        let update = Update::patched(Version::new("v1"), vec![]);
        assert_eq!(rendered(&update), "Version: \"v1\"\r\nPatches: 0\r\n\r\n");
    }

    #[test]
    fn empty_snapshot_still_declares_a_zero_length_body() {
        let update = Update::snapshot(Version::new("v1"), "");
        assert_eq!(
            rendered(&update),
            "Version: \"v1\"\r\nContent-Length: 0\r\n\r\n\r\n"
        );
    }

    #[test]
    fn heartbeat_is_a_bare_crlf() {
        assert_eq!(&format_heartbeat()[..], b"\r\n");
    }

    #[test]
    fn content_type_and_merge_type_are_emitted() {
        let update = Update::snapshot(Version::new("v1"), "{}")
            .with_content_type("application/json")
            .with_merge_type("simpleton");
        let s = rendered(&update);
        assert!(s.contains("Content-Type: application/json\r\n"));
        assert!(s.contains("Merge-Type: simpleton\r\n"));
    }
}
