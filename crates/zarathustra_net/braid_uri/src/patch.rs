//! Braid-HTTP version patch construction.
//!
//! A [`BraidPatch`] bundles a serialised [`ChessMessage`] with the Braid
//! version/parents needed for the `Version` and `Parents` HTTP headers.

use crate::error::BraidUriError;
use crate::message::{ChessMessage, MovePayload};
use hex;
use sha2::{Digest, Sha256};

/// A Braid-HTTP version patch ready to be PUT to a chess resource.
#[derive(Debug, Clone)]
pub struct BraidPatch {
    /// Version hash for this patch (16 hex chars, derived from FEN + move number).
    pub version: String,
    /// Parent versions that this patch follows.
    pub parents: Vec<String>,
    /// JSON body of the [`ChessMessage`].
    pub body: String,
}

impl BraidPatch {
    /// Build a patch from a [`MovePayload`] and the previous version hash.
    pub fn from_move(payload: &MovePayload, parent_version: &str) -> Result<Self, BraidUriError> {
        let version = version_hash(&payload.fen_after, payload.move_number);
        let body = serde_json::to_string(&ChessMessage::Move(payload.clone()))?;
        Ok(BraidPatch {
            version,
            parents: vec![parent_version.to_string()],
            body,
        })
    }

    /// Build a patch for a non-move message (resign, draw offer, etc.).
    pub fn from_message(
        msg: &ChessMessage,
        parent_version: &str,
        version_seed: &str,
    ) -> Result<Self, BraidUriError> {
        let version = version_hash(version_seed, 0);
        let body = serde_json::to_string(msg)?;
        Ok(BraidPatch {
            version,
            parents: vec![parent_version.to_string()],
            body,
        })
    }

    /// Return the `Version` header value (e.g. `["{version}"]`).
    pub fn version_header(&self) -> String {
        format!("[\"{}\"]", self.version)
    }

    /// Return the `Parents` header value (e.g. `["{p1}", "{p2}"]`).
    pub fn parents_header(&self) -> String {
        let parts: Vec<String> = self.parents.iter().map(|p| format!("\"{}\"", p)).collect();
        format!("[{}]", parts.join(", "))
    }
}

/// Derive a 16-char hex version hash from a FEN string and move number.
///
/// The hash is deterministic: same FEN + move number always produces the same version.
pub fn version_hash(fen: &str, move_number: u32) -> String {
    let input = format!("{}:{}", fen, move_number);
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(&digest[..8]) // 8 bytes = 16 hex chars
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MovePayload;

    const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";

    #[test]
    fn version_hash_is_deterministic() {
        let v1 = version_hash(START_FEN, 1);
        let v2 = version_hash(START_FEN, 1);
        assert_eq!(v1, v2);
        assert_eq!(v1.len(), 16);
    }

    #[test]
    fn version_hash_changes_with_move_number() {
        let v1 = version_hash(START_FEN, 1);
        let v2 = version_hash(START_FEN, 2);
        assert_ne!(v1, v2);
    }

    #[test]
    fn patch_from_move() {
        let payload = MovePayload::from_uci("e2e4", START_FEN, 1, "alice");
        let patch = BraidPatch::from_move(&payload, "root").unwrap();
        assert_eq!(patch.parents, vec!["root"]);
        assert_eq!(patch.version.len(), 16);
        assert!(patch.body.contains("\"type\":\"move\""));
    }

    #[test]
    fn patch_headers() {
        let payload = MovePayload::from_uci("e2e4", START_FEN, 1, "alice");
        let patch = BraidPatch::from_move(&payload, "root").unwrap();
        assert!(patch.version_header().starts_with("[\""));
        assert!(patch.parents_header().contains("\"root\""));
    }
}
