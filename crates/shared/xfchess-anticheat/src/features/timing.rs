use crate::config::AcConfig;
use crate::types::{Complexity, MoveRecord, PlyEval};

/// Compute the timing anomaly signal for one side's plies.
///
/// Returns a value in [0.0, 1.0].
/// A high value means the player made many suspiciously fast moves on Complex
/// positions.  Plies where the player was in time trouble are excluded.
pub fn timing_anomaly(
    plies: &[PlyEval],
    moves: &[MoveRecord],
    cfg: &AcConfig,
) -> f64 {
    let mut complex_count = 0u32;
    let mut fast_count = 0u32;

    for eval in plies {
        if eval.complexity != Complexity::Complex {
            continue;
        }

        // Find the matching move record for clock context
        let latency_ms = moves
            .iter()
            .find(|m| m.ply == eval.ply)
            .map(|m| m.latency_ms)
            .unwrap_or(eval.latency_ms);

        // Skip if player was in time trouble (clock below threshold)
        // We don't have per-ply clock data from the server yet, so we proxy
        // it: if the move was made very quickly AND it's the endgame (ply > 60),
        // we are more lenient.
        let endgame_leniency = eval.ply > 60;
        let effective_threshold = if endgame_leniency {
            cfg.timing_fast_threshold_ms / 2
        } else {
            cfg.timing_fast_threshold_ms
        };

        complex_count += 1;
        if latency_ms < effective_threshold {
            fast_count += 1;
        }
    }

    if complex_count == 0 {
        return 0.0;
    }

    fast_count as f64 / complex_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AcConfig;
    use crate::types::{Complexity, MoveRecord, PlyEval};

    fn eval(ply: u32, complexity: Complexity, latency_ms: u32) -> PlyEval {
        PlyEval {
            ply,
            move_uci: "e2e4".into(),
            top1_cp: 100,
            top2_cp: 0,
            cpl: 0,
            is_t1: true,
            complexity,
            latency_ms,
        }
    }

    fn mv(ply: u32, latency_ms: u32) -> MoveRecord {
        MoveRecord {
            ply,
            move_uci: "e2e4".into(),
            fen_after: String::new(),
            signed_at_ms: 0,
            latency_ms,
        }
    }

    #[test]
    fn all_fast_complex_is_suspicious() {
        let cfg = AcConfig::default();
        let plies = vec![
            eval(4, Complexity::Complex, 300),
            eval(6, Complexity::Complex, 400),
            eval(8, Complexity::Complex, 200),
        ];
        let moves = vec![mv(4, 300), mv(6, 400), mv(8, 200)];
        let signal = timing_anomaly(&plies, &moves, &cfg);
        assert_eq!(signal, 1.0);
    }

    #[test]
    fn all_slow_complex_is_clean() {
        let cfg = AcConfig::default();
        let plies = vec![
            eval(4, Complexity::Complex, 5_000),
            eval(6, Complexity::Complex, 8_000),
        ];
        let moves = vec![mv(4, 5_000), mv(6, 8_000)];
        let signal = timing_anomaly(&plies, &moves, &cfg);
        assert_eq!(signal, 0.0);
    }

    #[test]
    fn forced_plies_ignored() {
        let cfg = AcConfig::default();
        let plies = vec![
            eval(2, Complexity::Forced, 100),
            eval(4, Complexity::Forced, 100),
        ];
        let signal = timing_anomaly(&plies, &[], &cfg);
        assert_eq!(signal, 0.0);
    }
}
