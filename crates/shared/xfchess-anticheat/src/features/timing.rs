use crate::config::AcConfig;
use crate::types::{Complexity, MoveRecord, PlyEval, TimingSource};

/// Minimum fraction of a side's moves carrying client think times for the
/// side to use `TimingSource::Client`.
const CLIENT_COVERAGE_MIN: f64 = 0.8;
/// Server timestamps are plausible only if the game's wall time averages at
/// least this many ms per ply — batch submission compresses it to ~0.
const MIN_PLAUSIBLE_MS_PER_PLY: u64 = 500;

/// Resolves where one side's timing data should come from.
/// `parity` 0 = white (even move indices), 1 = black.
pub fn source_for(moves: &[MoveRecord], parity: usize) -> TimingSource {
    let side: Vec<&MoveRecord> = moves
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 2 == parity)
        .map(|(_, m)| m)
        .collect();
    if side.is_empty() {
        return TimingSource::None;
    }
    let with_think = side.iter().filter(|m| m.think_ms.is_some()).count();
    if with_think as f64 / side.len() as f64 >= CLIENT_COVERAGE_MIN {
        return TimingSource::Client;
    }
    if server_timing_plausible(moves) {
        return TimingSource::Server;
    }
    TimingSource::None
}

/// True when the server-observed game wall time is believable for the move
/// count. Batch-submitted games (all moves POSTed in a loop at game end)
/// fail this and must not feed the timing signals — they'd read as a player
/// who moved instantly every ply.
pub fn server_timing_plausible(moves: &[MoveRecord]) -> bool {
    if moves.len() < 2 {
        return false;
    }
    let first = moves.first().map(|m| m.signed_at_ms).unwrap_or(0);
    let last = moves.last().map(|m| m.signed_at_ms).unwrap_or(0);
    last.saturating_sub(first) >= moves.len() as u64 * MIN_PLAUSIBLE_MS_PER_PLY
}

/// Effective think time for one move under the resolved source. Missing or
/// unusable data maps to `u32::MAX` ("slow"), which can never read as
/// suspiciously fast — absence of timing must not manufacture suspicion.
pub fn effective_latency(m: &MoveRecord, source: TimingSource) -> u32 {
    match source {
        TimingSource::Client => m.think_ms.unwrap_or(u32::MAX),
        TimingSource::Server => m.latency_ms,
        TimingSource::None => u32::MAX,
    }
}

/// Compute the timing anomaly signal for one side's plies.
///
/// Returns a value in [0.0, 1.0].
/// A high value means the player made many suspiciously fast moves on Complex
/// positions.  Plies where the player was in time trouble are excluded.
pub fn timing_anomaly(
    plies: &[PlyEval],
    moves: &[MoveRecord],
    source: TimingSource,
    cfg: &AcConfig,
) -> f64 {
    // No trustworthy timing for this side — disable the signal entirely
    // rather than feed it garbage (batch-collapsed or absent data).
    if source == TimingSource::None {
        return 0.0;
    }

    let mut complex_count = 0u32;
    let mut fast_count = 0u32;

    for eval in plies {
        if eval.complexity != Complexity::Complex {
            continue;
        }

        // Resolve this move's think time under the side's timing source.
        let latency_ms = moves
            .iter()
            .find(|m| m.ply == eval.ply)
            .map(|m| effective_latency(m, source))
            .unwrap_or(u32::MAX);

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
            blurred: false,
            think_ms: None,
        }
    }

    /// Builds a side's moves carrying client think times at the given signed_at
    /// spacing, so the resolver picks `Client` and the wall clock is plausible.
    fn mv_client(ply: u32, think_ms: u32, signed_at_ms: u64) -> MoveRecord {
        MoveRecord {
            ply,
            move_uci: "e2e4".into(),
            fen_after: String::new(),
            signed_at_ms,
            latency_ms: 0,
            blurred: false,
            think_ms: Some(think_ms),
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
        let signal = timing_anomaly(&plies, &moves, TimingSource::Server, &cfg);
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
        let signal = timing_anomaly(&plies, &moves, TimingSource::Server, &cfg);
        assert_eq!(signal, 0.0);
    }

    #[test]
    fn forced_plies_ignored() {
        let cfg = AcConfig::default();
        let plies = vec![
            eval(2, Complexity::Forced, 100),
            eval(4, Complexity::Forced, 100),
        ];
        let signal = timing_anomaly(&plies, &[], TimingSource::Server, &cfg);
        assert_eq!(signal, 0.0);
    }

    #[test]
    fn none_source_disables_signal() {
        let cfg = AcConfig::default();
        // Fast moves that would read as 1.0 under Server are silenced by None.
        let plies = vec![eval(4, Complexity::Complex, 100)];
        let moves = vec![mv(4, 100)];
        assert_eq!(timing_anomaly(&plies, &moves, TimingSource::None, &cfg), 0.0);
    }

    #[test]
    fn batch_collapsed_timestamps_are_implausible() {
        // 40 plies all stamped within 1s of each other — the GameEndBatch case.
        let moves: Vec<MoveRecord> = (0..40)
            .map(|p| {
                let mut m = mv(p, 0);
                m.signed_at_ms = 1_700_000_000_000 + p as u64 * 10;
                m
            })
            .collect();
        assert!(!server_timing_plausible(&moves));
        assert_eq!(source_for(&moves, 0), TimingSource::None);
    }

    #[test]
    fn realtime_timestamps_are_plausible_server() {
        // Moves ~3s apart, no client think times → Server.
        let moves: Vec<MoveRecord> = (0..40)
            .map(|p| {
                let mut m = mv(p, 3_000);
                m.signed_at_ms = 1_700_000_000_000 + p as u64 * 3_000;
                m
            })
            .collect();
        assert!(server_timing_plausible(&moves));
        assert_eq!(source_for(&moves, 0), TimingSource::Server);
    }

    #[test]
    fn client_think_times_preferred() {
        // White (even plies) carries think times → Client even though the
        // wall clock is batch-collapsed.
        let moves: Vec<MoveRecord> = (0..40)
            .map(|p| {
                if p % 2 == 0 {
                    mv_client(p, 4_000, 1_700_000_000_000 + p as u64 * 5)
                } else {
                    let mut m = mv(p, 0);
                    m.signed_at_ms = 1_700_000_000_000 + p as u64 * 5;
                    m
                }
            })
            .collect();
        assert_eq!(source_for(&moves, 0), TimingSource::Client);
        // Black has no think times and the wall clock is collapsed → None.
        assert_eq!(source_for(&moves, 1), TimingSource::None);
    }

    #[test]
    fn effective_latency_per_source() {
        let m = mv_client(0, 4_000, 0);
        assert_eq!(effective_latency(&m, TimingSource::Client), 4_000);
        let s = mv(0, 1_234);
        assert_eq!(effective_latency(&s, TimingSource::Server), 1_234);
        assert_eq!(effective_latency(&s, TimingSource::None), u32::MAX);
    }
}
