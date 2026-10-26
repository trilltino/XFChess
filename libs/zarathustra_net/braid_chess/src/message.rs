#[cfg(test)]
use anyhow::Context as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChessMessage {
    Move(MovePayload),
    Resign {
        player: String,
    },
    /// A draw offer, its acceptance or its refusal. `ply` is the position it was
    /// made in (a move makes an offer lapse) and `at_ms` when, so a repeat of one
    /// action dedupes while a later offer in the same position stays a new event.
    /// Logs written before these fields existed read as `0`.
    OfferDraw {
        player: String,
        #[serde(default)]
        ply: u32,
        #[serde(default)]
        at_ms: u64,
    },
    AcceptDraw {
        player: String,
        #[serde(default)]
        ply: u32,
        #[serde(default)]
        at_ms: u64,
    },
    DeclineDraw {
        player: String,
        #[serde(default)]
        ply: u32,
        #[serde(default)]
        at_ms: u64,
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

impl ChessMessage {
    /// Identity of a draw action; the Braid version of it is a hash of this. `None`
    /// for every other kind of message.
    pub fn draw_key(&self) -> Option<String> {
        let (kind, player, ply, at_ms) = match self {
            Self::OfferDraw { player, ply, at_ms } => ("offer_draw", player, ply, at_ms),
            Self::AcceptDraw { player, ply, at_ms } => ("accept_draw", player, ply, at_ms),
            Self::DeclineDraw { player, ply, at_ms } => ("decline_draw", player, ply, at_ms),
            _ => return None,
        };
        Some(format!("{kind}:{player}:{ply}:{at_ms}"))
    }
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
    /// The mover's base58 signature over [`crate::move_auth::message`] for
    /// this move, so the other participant can relay it. Older clients omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
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
            auth: None,
        }
    }

    pub fn with_auth(mut self, auth: Option<String>) -> Self {
        self.auth = auth;
        self
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
    fn chess_message_serde_roundtrip() -> anyhow::Result<()> {
        let msg = ChessMessage::Move(MovePayload::from_uci("d2d4", "fen", 2, "alice"));
        let json = serde_json::to_string(&msg).context("required operation value")?;
        assert!(json.contains("\"type\":\"move\""));
        let decoded: ChessMessage =
            serde_json::from_str(&json).context("required operation value")?;
        if let ChessMessage::Move(p) = decoded {
            assert_eq!(p.uci, "d2d4");
        } else {
            panic!("Wrong variant");
        }
        Ok(())
    }

    #[test]
    fn resign_message() -> anyhow::Result<()> {
        let msg = ChessMessage::Resign {
            player: "bob".to_string(),
        };
        let json = serde_json::to_string(&msg).context("required operation value")?;
        assert!(json.contains("\"type\":\"resign\""));
        Ok(())
    }

    #[test]
    fn draw_messages_from_older_logs_still_decode() -> anyhow::Result<()> {
        let decoded: ChessMessage =
            serde_json::from_str(r#"{"type":"offer_draw","player":"white"}"#)
                .context("required operation value")?;
        assert!(matches!(
            decoded,
            ChessMessage::OfferDraw {
                ply: 0,
                at_ms: 0,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn a_later_offer_in_the_same_position_has_a_different_key() {
        let offer = |at_ms| ChessMessage::OfferDraw {
            player: "white".into(),
            ply: 10,
            at_ms,
        };
        assert_eq!(offer(5).draw_key(), offer(5).draw_key());
        assert_ne!(offer(5).draw_key(), offer(6).draw_key());
        assert!(ChessMessage::Resign { player: "x".into() }
            .draw_key()
            .is_none());
    }

    #[test]
    fn engine_hint_serde() -> anyhow::Result<()> {
        let hint = ChessMessage::EngineAnalysis(EngineHint {
            depth: 20,
            score_cp: 42,
            mate_in: None,
            pv: vec!["e2e4".into(), "e7e5".into()],
            best_move: "e2e4".into(),
        });
        let json = serde_json::to_string(&hint).context("required operation value")?;
        let decoded: ChessMessage =
            serde_json::from_str(&json).context("required operation value")?;
        if let ChessMessage::EngineAnalysis(h) = decoded {
            assert_eq!(h.depth, 20);
            assert_eq!(h.best_move, "e2e4");
        } else {
            panic!("Wrong variant");
        }
        Ok(())
    }
}
