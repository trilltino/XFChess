//! Typed chess game messages carried over Braid-HTTP.
//!
//! Each [`ChessMessage`] variant represents a distinct game event.  
//! Messages are serialised as JSON and used as the body of Braid PUT requests.

use serde::{Deserialize, Serialize};

/// Top-level enum for all chess game events.
///
/// Serialised with a `"type"` discriminant field for easy JSON parsing on the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChessMessage {
    /// A piece was moved.
    Move(MovePayload),
    /// A player resigned.
    Resign { player: String },
    /// A player offered a draw.
    OfferDraw { player: String },
    /// A player accepted a draw offer.
    AcceptDraw { player: String },
    /// A player declined a draw offer.
    DeclineDraw { player: String },
    /// Clock state broadcast.
    Clock(ClockState),
    /// Stockfish analysis hint (streamed per depth increment).
    EngineAnalysis(EngineHint),
}

/// Payload for a chess move event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovePayload {
    /// Source square in algebraic notation (e.g. `"e2"`).
    pub from: String,
    /// Destination square in algebraic notation (e.g. `"e4"`).
    pub to: String,
    /// Promotion piece, if any (`'q'`, `'r'`, `'b'`, `'n'`).
    pub promotion: Option<char>,
    /// Full UCI notation (e.g. `"e2e4"` or `"e7e8q"`).
    pub uci: String,
    /// Complete FEN string after this move was applied.
    pub fen_after: String,
    /// 1-indexed move number (increments after Black's move).
    pub move_number: u32,
    /// Player who made this move (username or peer ID).
    pub player: String,
}

impl MovePayload {
    /// Build a [`MovePayload`] from a UCI string and the resulting FEN.
    pub fn from_uci(
        uci: impl Into<String>,
        fen_after: impl Into<String>,
        move_number: u32,
        player: impl Into<String>,
    ) -> Self {
        let uci_str: String = uci.into();
        let from = uci_str[..2].to_string();
        let to = uci_str[2..4].to_string();
        let promotion = uci_str.chars().nth(4);
        MovePayload {
            from,
            to,
            promotion,
            uci: uci_str,
            fen_after: fen_after.into(),
            move_number,
            player: player.into(),
        }
    }
}

/// Clock state update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockState {
    /// White's remaining time in milliseconds.
    pub white_ms: u64,
    /// Black's remaining time in milliseconds.
    pub black_ms: u64,
    /// Timestamp (ms since Unix epoch) when this snapshot was taken.
    pub timestamp_ms: u64,
}

/// Stockfish engine analysis hint, streamed per depth increment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHint {
    /// Search depth that produced this hint.
    pub depth: u8,
    /// Evaluation score in centipawns (positive = good for the side to move).
    pub score_cp: i32,
    /// Mate-in-N, if a forced mate was found.
    pub mate_in: Option<i32>,
    /// Principal variation – sequence of best moves in UCI notation.
    pub pv: Vec<String>,
    /// Best move in UCI notation.
    pub best_move: String,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_payload_roundtrip() {
        let mv = MovePayload::from_uci(
            "e2e4",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            1,
            "alice",
        );
        assert_eq!(mv.from, "e2");
        assert_eq!(mv.to, "e4");
        assert!(mv.promotion.is_none());
        assert_eq!(mv.uci, "e2e4");
    }

    #[test]
    fn promotion_payload() {
        let mv = MovePayload::from_uci("e7e8q", "some fen", 15, "bob");
        assert_eq!(mv.promotion, Some('q'));
    }

    #[test]
    fn chess_message_serde_roundtrip() {
        let msg = ChessMessage::Move(MovePayload::from_uci("d2d4", "fen", 2, "alice"));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"move\""));
        let decoded: ChessMessage = serde_json::from_str(&json).unwrap();
        if let ChessMessage::Move(p) = decoded {
            assert_eq!(p.uci, "d2d4");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn resign_message() {
        let msg = ChessMessage::Resign {
            player: "bob".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"resign\""));
    }

    #[test]
    fn engine_hint_serde() {
        let hint = ChessMessage::EngineAnalysis(EngineHint {
            depth: 20,
            score_cp: 42,
            mate_in: None,
            pv: vec!["e2e4".into(), "e7e5".into()],
            best_move: "e2e4".into(),
        });
        let json = serde_json::to_string(&hint).unwrap();
        let decoded: ChessMessage = serde_json::from_str(&json).unwrap();
        if let ChessMessage::EngineAnalysis(h) = decoded {
            assert_eq!(h.depth, 20);
            assert_eq!(h.best_move, "e2e4");
        } else {
            panic!("Wrong variant");
        }
    }
}
