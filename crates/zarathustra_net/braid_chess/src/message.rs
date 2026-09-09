use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChessMessage {
    Move(MovePayload),
    Resign {
        player: String,
    },
    OfferDraw {
        player: String,
    },
    AcceptDraw {
        player: String,
    },
    DeclineDraw {
        player: String,
    },
    Clock(ClockState),
    EngineAnalysis(EngineHint),
    Chat(ChatPayload),
    SessionInfo {
        player_pubkey: String,
        session_pubkey: String,
        signing_pubkey: String,
        expires_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPayload {
    pub player: String,
    pub text: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovePayload {
    pub from: String,
    pub to: String,
    pub promotion: Option<char>,
    pub uci: String,
    pub fen_after: String,
    pub move_number: u32,
    pub player: String,
}

impl MovePayload {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockState {
    pub white_ms: u64,
    pub black_ms: u64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHint {
    pub depth: u8,
    pub score_cp: i32,
    pub mate_in: Option<i32>,
    pub pv: Vec<String>,
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
