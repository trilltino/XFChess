use crate::error::BraidChessError;
use crate::message::{ChessMessage, MovePayload};
use hex;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct BraidPatch {
    pub version: String,
    pub parents: Vec<String>,
    pub body: String,
}

impl BraidPatch {
    pub fn from_move(payload: &MovePayload, parent_version: &str) -> Result<Self, BraidChessError> {
        let version = version_hash(&payload.fen_after, payload.move_number);
        let body = serde_json::to_string(&ChessMessage::Move(payload.clone()))?;
        Ok(BraidPatch {
            version,
            parents: vec![parent_version.to_string()],
            body,
        })
    }

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

    pub fn version_header(&self) -> String {
        format!("[\"{}\"]", self.version)
    }

    pub fn parents_header(&self) -> String {
        let parts: Vec<String> = self.parents.iter().map(|p| format!("\"{}\"", p)).collect();
        format!("[{}]", parts.join(", "))
    }
}

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
