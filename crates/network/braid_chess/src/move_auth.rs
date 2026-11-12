//! What a player signs to authorise one of their own moves, so the other
//! participant can submit it to the backend for them.
//!
//! The backend holds each player's session key and signs `record_move` for
//! whichever player a request names, so a move relayed by the opponent needs
//! proof that the named player chose it. The player signs [`message`] with the
//! per-device key registered for their seat. The signature covers the game,
//! the ply (the on-chain move nonce), the move and the move it answers, so it
//! cannot be replayed at another ply or after a different opponent move.

const DOMAIN: &[u8] = b"xfchess/move-auth/v1";

/// The bytes a player signs to authorise `uci` as ply `ply` of `game_id`,
/// played in reply to `previous_uci` (`None` for the first move).
pub fn message(game_id: u64, ply: u64, uci: &str, previous_uci: Option<&str>) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DOMAIN.len() + 16 + 2 * 7);
    msg.extend_from_slice(DOMAIN);
    msg.extend_from_slice(&game_id.to_le_bytes());
    msg.extend_from_slice(&ply.to_le_bytes());
    push_move(&mut msg, uci);
    push_move(&mut msg, previous_uci.unwrap_or(""));
    msg
}

/// Length-prefixed, so a move and the one before it cannot be re-split.
fn push_move(msg: &mut Vec<u8>, uci: &str) {
    let uci = uci.trim().to_ascii_lowercase();
    let len = u16::try_from(uci.len()).unwrap_or(u16::MAX);
    msg.extend_from_slice(&len.to_le_bytes());
    msg.extend_from_slice(&uci.as_bytes()[..usize::from(len)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_game_ply_move_and_reply_context() {
        let base = message(7, 2, "e7e5", Some("e2e4"));
        assert_ne!(base, message(8, 2, "e7e5", Some("e2e4")));
        assert_ne!(base, message(7, 4, "e7e5", Some("e2e4")));
        assert_ne!(base, message(7, 2, "e7e6", Some("e2e4")));
        assert_ne!(base, message(7, 2, "e7e5", Some("d2d4")));
    }

    #[test]
    fn move_boundaries_cannot_be_shifted() {
        assert_ne!(
            message(1, 3, "e7e8q", Some("a2a3")),
            message(1, 3, "e7e8", Some("qa2a3"))
        );
    }

    #[test]
    fn notation_case_and_whitespace_do_not_change_the_message() {
        assert_eq!(
            message(1, 5, " E7E8Q ", Some("A2A3")),
            message(1, 5, "e7e8q", Some("a2a3"))
        );
    }
}
