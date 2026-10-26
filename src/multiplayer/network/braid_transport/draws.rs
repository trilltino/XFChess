//! Draw offers and answers arriving over the Braid log.
//!
//! Gossip carries them live; the log carries them across a disconnect, so an
//! answer made while this client was offline is still delivered when it returns.

use crate::game::systems::network_move::DRAW_OFFER_TTL;
use braid_chess::ChessMessage;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RemoteDraw {
    Offer { player: String, ply: u32 },
    Accept { player: String, ply: u32 },
    Decline { player: String, ply: u32 },
}

/// What a draw message from the log asks of this client, or `None` when it is no
/// concern of ours:
/// * our own action coming back (after a restart the self-echo filter is empty);
/// * an offer older than its prompt lives: a replay must not raise a prompt nobody
///   is waiting on. Answers are never aged out, since a late one is still real;
///   whether it matches an offer of ours is checked where the offer is kept.
pub(super) fn remote_draw(
    message: &ChessMessage,
    local_player: Option<&str>,
    now_ms: u64,
) -> Option<RemoteDraw> {
    let (player, ply, at_ms, make): (_, _, _, fn(String, u32) -> RemoteDraw) = match message {
        ChessMessage::OfferDraw { player, ply, at_ms } => (player, *ply, *at_ms, |player, ply| {
            RemoteDraw::Offer { player, ply }
        }),
        ChessMessage::AcceptDraw { player, ply, .. } => (player, *ply, 0, |player, ply| {
            RemoteDraw::Accept { player, ply }
        }),
        ChessMessage::DeclineDraw { player, ply, .. } => (player, *ply, 0, |player, ply| {
            RemoteDraw::Decline { player, ply }
        }),
        _ => return None,
    };
    if local_player == Some(player.as_str()) {
        return None;
    }
    if matches!(message, ChessMessage::OfferDraw { .. }) {
        let age_ms = now_ms.saturating_sub(at_ms);
        // `at_ms == 0` is a log written before offers were timestamped.
        if at_ms == 0 || age_ms > DRAW_OFFER_TTL.as_millis() as u64 {
            return None;
        }
    }
    Some(make(player.clone(), ply))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(player: &str, ply: u32, at_ms: u64) -> ChessMessage {
        ChessMessage::OfferDraw {
            player: player.into(),
            ply,
            at_ms,
        }
    }

    #[test]
    fn a_fresh_offer_from_the_opponent_is_surfaced_with_its_position() {
        assert_eq!(
            remote_draw(&offer("white", 12, 100_000), Some("black"), 110_000),
            Some(RemoteDraw::Offer {
                player: "white".into(),
                ply: 12
            })
        );
    }

    #[test]
    fn our_own_action_coming_back_is_ignored() {
        assert_eq!(
            remote_draw(&offer("black", 12, 100_000), Some("black"), 100_500),
            None
        );
    }

    #[test]
    fn a_replayed_old_offer_raises_no_prompt() {
        let ttl = DRAW_OFFER_TTL.as_millis() as u64;
        assert!(remote_draw(&offer("white", 12, 1_000), Some("black"), 1_000 + ttl).is_some());
        assert_eq!(
            remote_draw(&offer("white", 12, 1_000), Some("black"), 1_000 + ttl + 1),
            None
        );
        assert_eq!(remote_draw(&offer("white", 12, 0), Some("black"), 5), None);
    }

    #[test]
    fn a_late_answer_is_still_delivered() {
        let accept = ChessMessage::AcceptDraw {
            player: "white".into(),
            ply: 12,
            at_ms: 1,
        };
        assert_eq!(
            remote_draw(&accept, Some("black"), 10_000_000),
            Some(RemoteDraw::Accept {
                player: "white".into(),
                ply: 12
            })
        );
        let decline = ChessMessage::DeclineDraw {
            player: "white".into(),
            ply: 12,
            at_ms: 1,
        };
        assert!(remote_draw(&decline, Some("black"), 10_000_000).is_some());
    }

    #[test]
    fn other_messages_are_not_draws() {
        let resign = ChessMessage::Resign {
            player: "white".into(),
        };
        assert_eq!(remote_draw(&resign, Some("black"), 0), None);
    }
}
