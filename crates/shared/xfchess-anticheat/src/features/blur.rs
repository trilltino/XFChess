//! Blur signal — client-reported window-focus loss per move.
//!
//! The strongest pre-engine signal Lichess uses: a player alt-tabbing to an
//! engine leaves the game window before (almost) every move. The flag is
//! reported by the player's own client (see `POST /telemetry/blur` in the
//! backend); absence of telemetry yields 0.0, so blur can only ever *add*
//! suspicion, never clear it.

use crate::types::MoveRecord;

/// Fraction of one side's moves carrying a blur flag (0.0–1.0).
/// `parity` 0 = white (even move indices), 1 = black.
pub fn blur_rate(moves: &[MoveRecord], parity: usize) -> f64 {
    let mut total = 0u32;
    let mut blurred = 0u32;
    for (i, m) in moves.iter().enumerate() {
        if i % 2 != parity {
            continue;
        }
        total += 1;
        if m.blurred {
            blurred += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    blurred as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(blurred: bool) -> MoveRecord {
        MoveRecord {
            ply: 0,
            move_uci: "e2e4".into(),
            fen_after: String::new(),
            signed_at_ms: 0,
            latency_ms: 1000,
            blurred,
            think_ms: None,
        }
    }

    #[test]
    fn per_side_rates() {
        // White blurs every move, black never.
        let moves: Vec<MoveRecord> = (0..20).map(|i| mv(i % 2 == 0)).collect();
        assert_eq!(blur_rate(&moves, 0), 1.0);
        assert_eq!(blur_rate(&moves, 1), 0.0);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(blur_rate(&[], 0), 0.0);
    }
}
