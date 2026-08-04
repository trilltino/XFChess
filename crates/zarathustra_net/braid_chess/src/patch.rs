//! Braid-HTTP version patch construction.
//!
//! A [`BraidPatch`] bundles a serialised [`ChessMessage`] with the Braid
//! version/parents needed for the `Version` and `Parents` HTTP headers.

use crate::error::BraidChessError;
use crate::message::{ChessMessage, MovePayload};
use hex;
use sha2::{Digest, Sha256};

/// A Braid-HTTP version patch ready to be PUT to a chess resource.
///
/// Chess moves are strictly linear — no player ever produces two candidate
/// next-moves from the same position — so `parents` here is always a single
/// entry. This is deliberately *not* a CRDT/OT merge structure: the upstream
/// Braid merge engine was removed from this codebase (see `crates/CLAUDE.md`)
/// in favor of a simple, disputable, content-addressed hash chain. A future
/// reader should not expect multi-parent merge semantics to appear here.
#[derive(Debug, Clone)]
pub struct BraidPatch {
    /// Version hash for this patch (64 hex chars — full SHA-256 of FEN +
    /// move number). Wide on purpose: once this backs a durable, disputable
    /// move log (not just an ephemeral P2P equivocation check), a collision
    /// needs to be cryptographically implausible, not merely "vanishingly
    /// unlikely for a single game's move count."
    pub version: String,
    /// The single preceding version this patch follows (see struct doc).
    pub parents: Vec<String>,
    /// JSON body of the [`ChessMessage`].
    pub body: String,
}

impl BraidPatch {
    /// Build a patch from a [`MovePayload`] and the previous version hash.
    pub fn from_move(payload: &MovePayload, parent_version: &str) -> Result<Self, BraidChessError> {
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
    ) -> Result<Self, BraidChessError> {
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

/// Derive a full-width (64-char hex) SHA-256 version hash from a FEN string
/// and move number.
///
/// The hash is deterministic: same FEN + move number always produces the
/// same version. Full-width rather than truncated: this value is both the
/// P2P causal-chain equivocation check (`CausalChainState::head_version` in
/// the game client) *and*, once wired up, the durable version key for the
/// backend's persisted move log — a collision there would silently merge
/// two distinct game states in a wagered game's dispute-evidence trail, so
/// the full 256-bit digest is used rather than a truncated prefix.
pub fn version_hash(fen: &str, move_number: u32) -> String {
    let input = format!("{}:{}", fen, move_number);
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest) // 32 bytes = 64 hex chars
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
        assert_eq!(v1.len(), 64, "expected full-width SHA-256 hex (64 chars)");
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
        assert_eq!(patch.version.len(), 64);
        assert!(patch.body.contains("\"type\":\"move\""));
    }

    #[test]
    fn parent_chain_is_single_linear_parent() {
        // Chess has no concurrent-move branching — every patch's `parents`
        // must be exactly one entry, never a multi-parent merge point.
        let payload = MovePayload::from_uci("e2e4", START_FEN, 1, "alice");
        let patch = BraidPatch::from_move(&payload, "some-parent-version").unwrap();
        assert_eq!(patch.parents.len(), 1);
        assert_eq!(patch.parents[0], "some-parent-version");
    }

    #[test]
    fn patch_headers() {
        let payload = MovePayload::from_uci("e2e4", START_FEN, 1, "alice");
        let patch = BraidPatch::from_move(&payload, "root").unwrap();
        assert!(patch.version_header().starts_with("[\""));
        assert!(patch.parents_header().contains("\"root\""));
    }
}
