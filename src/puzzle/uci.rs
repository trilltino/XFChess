//! Conversions between the backend's UCI move strings and the board's
//! `(file, rank)` coordinates.

use crate::game::events::NetworkMoveEvent;
use crate::rendering::pieces::PieceType;

/// A server-supplied move (`e2e4`, `e7e8q`) as a move event for the board.
pub fn uci_to_event(uci: &str) -> Option<NetworkMoveEvent> {
    let b = uci.trim().as_bytes();
    if !(4..=5).contains(&b.len()) {
        return None;
    }
    let file = |c: u8| i32::from(c.to_ascii_lowercase()) - i32::from(b'a');
    let rank = |c: u8| i32::from(c) - i32::from(b'1');
    let (ff, fr, tf, tr) = (file(b[0]), rank(b[1]), file(b[2]), rank(b[3]));
    if ![ff, fr, tf, tr].iter().all(|v| (0..8).contains(v)) {
        return None;
    }
    let promotion = match b.get(4).map(u8::to_ascii_lowercase) {
        None => None,
        Some(c @ (b'q' | b'r' | b'b' | b'n')) => Some(c as char),
        Some(_) => return None,
    };
    Some(NetworkMoveEvent {
        from: (ff as u8, fr as u8),
        to: (tf as u8, tr as u8),
        promotion,
        expected_fen: None,
        dedup_version: None,
    })
}

/// The player's move in UCI form, as the backend expects it.
pub fn coords_to_uci(from: (u8, u8), to: (u8, u8), promotion: Option<PieceType>) -> String {
    let square = |(file, rank): (u8, u8)| format!("{}{}", (b'a' + file) as char, rank + 1);
    let mut uci = format!("{}{}", square(from), square(to));
    if let Some(c) = promotion.and_then(promotion_char) {
        uci.push(c);
    }
    uci
}

/// A UCI square (`e4`) as board coordinates.
pub fn square(name: &str) -> Option<(u8, u8)> {
    let event = uci_to_event(&format!("{name}{name}"))?;
    Some(event.from)
}

fn promotion_char(piece: PieceType) -> Option<char> {
    match piece {
        PieceType::Queen => Some('q'),
        PieceType::Rook => Some('r'),
        PieceType::Bishop => Some('b'),
        PieceType::Knight => Some('n'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_promotion_moves() {
        let e = uci_to_event("e2e4").unwrap();
        assert_eq!((e.from, e.to, e.promotion), ((4, 1), (4, 3), None));
        let p = uci_to_event("a7a8Q").unwrap();
        assert_eq!((p.from, p.to, p.promotion), ((0, 6), (0, 7), Some('q')));
    }

    #[test]
    fn rejects_malformed_moves() {
        for bad in ["", "e2", "e2e9", "i2e4", "e7e8k", "e2e4qq"] {
            assert!(uci_to_event(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn round_trips_through_coordinates() {
        assert_eq!(coords_to_uci((4, 1), (4, 3), None), "e2e4");
        assert_eq!(
            coords_to_uci((7, 6), (7, 7), Some(PieceType::Knight)),
            "h7h8n"
        );
        assert_eq!(coords_to_uci((0, 0), (1, 0), Some(PieceType::King)), "a1b1");
        assert_eq!(square("h8"), Some((7, 7)));
        assert_eq!(square("z1"), None);
    }
}
