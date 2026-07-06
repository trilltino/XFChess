//! T0 pre-engine screen — pure timing heuristics, no Stockfish.
//!
//! Runs inline at enqueue time on every finished game so that free, casual
//! games can skip full engine analysis entirely. Stakes-bearing games (wager
//! or tournament) always get full analysis regardless of this screen; for
//! everything else, only games this screen marks suspicious enter the
//! Stockfish queue.
//!
//! Signals are timing-only (the same family Lichess leans on before engine
//! evidence): abnormally flat move times (humans vary, engine relayers
//! metronome) and the total absence of snap moves over a long game.

use crate::types::GameRecord;

/// Minimum usable move-time samples per side; below this the screen abstains.
const MIN_SAMPLES: usize = 12;
/// Opening plies skipped per side (book moves have meaningless timing).
const BOOK_PLIES_PER_SIDE: usize = 4;
/// Mean move time must be at least this for flatness to mean anything —
/// uniformly fast bullet-style moves are normal.
const FLAT_MIN_MEAN_MS: f64 = 3_000.0;
/// A move under this is a "snap" move; engine users rarely produce any.
const SNAP_MOVE_MS: u32 = 1_500;
/// Per-side score at or above this marks the game suspicious.
const SUSPICIOUS_SCORE: f64 = 0.5;

/// Outcome of the T0 screen for one game.
#[derive(Debug, Clone, Copy)]
pub struct ScreenResult {
    /// Heuristic suspicion score per side, 0.0–1.0.
    pub white_score: f64,
    pub black_score: f64,
    /// True when either side scores >= [`SUSPICIOUS_SCORE`].
    pub suspicious: bool,
}

/// Pure timing screen over the recorded move latencies. Never produces a
/// verdict on its own — it only decides whether a free game is worth engine
/// time.
pub fn t0_screen(game: &GameRecord) -> ScreenResult {
    let white_score = side_score(game, 0);
    let black_score = side_score(game, 1);
    ScreenResult {
        white_score,
        black_score,
        suspicious: white_score >= SUSPICIOUS_SCORE || black_score >= SUSPICIOUS_SCORE,
    }
}

/// Scores one side's move times. `parity` 0 = white (even move indices).
fn side_score(game: &GameRecord, parity: usize) -> f64 {
    use crate::features::timing::{effective_latency, source_for};

    let mut score: f64 = 0.0;

    // Timing heuristics only run when this side has trustworthy timing
    // (client think times, or a non-batched server wall clock). Batch-
    // collapsed games resolve to None and contribute no timing score — they
    // would otherwise read as "every move instant" and false-positive.
    let source = source_for(&game.moves, parity);
    if source != crate::types::TimingSource::None {
        let latencies: Vec<f64> = game
            .moves
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == parity)
            .skip(BOOK_PLIES_PER_SIDE)
            .map(|(_, m)| effective_latency(m, source) as f64)
            .collect();

        if latencies.len() >= MIN_SAMPLES {
            let n = latencies.len() as f64;
            let mean = latencies.iter().sum::<f64>() / n;
            if mean > 0.0 {
                let variance = latencies.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n;
                let cv = variance.sqrt() / mean;

                // Flat move times: humans think longer on hard positions; a
                // coefficient of variation this low over a slow game is the
                // metronome signature.
                if mean >= FLAT_MIN_MEAN_MS {
                    if cv < 0.35 {
                        score += 0.7;
                    } else if cv < 0.5 {
                        score += 0.4;
                    }
                }

                // No snap moves at all: even strong humans bang out recaptures
                // and forced replies; an engine relay round-trips every move.
                let snaps = latencies
                    .iter()
                    .filter(|l| **l < SNAP_MOVE_MS as f64)
                    .count();
                if snaps == 0 {
                    score += 0.3;
                }
            }
        }
    }

    // Client-reported blur: alt-tabbing before most moves is the strongest
    // pre-engine signal there is — send the game to full analysis. Always
    // evaluated, independent of timing source.
    let blur = crate::features::blur::blur_rate(&game.moves, parity);
    if blur >= 0.7 {
        score += 0.5;
    } else if blur >= 0.4 {
        score += 0.25;
    }

    score.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GameContext, GameRecord, GameResult, MoveRecord, PlayerRef, TimeControl};

    /// Builds a game where white's latencies follow `white_ms` cyclically and
    /// black's follow `black_ms`. Timestamps accumulate from the latencies so
    /// the server wall clock is plausible (resolves to `TimingSource::Server`).
    fn game(plies: usize, white_ms: &[u32], black_ms: &[u32]) -> GameRecord {
        let mut clock: u64 = 1_700_000_000_000;
        let moves = (0..plies)
            .map(|i| {
                let lat = if i % 2 == 0 {
                    white_ms[(i / 2) % white_ms.len()]
                } else {
                    black_ms[(i / 2) % black_ms.len()]
                };
                clock += lat as u64;
                MoveRecord {
                    ply: i as u32,
                    move_uci: "e2e4".into(),
                    fen_after: String::new(),
                    signed_at_ms: clock,
                    latency_ms: lat,
                    blurred: false,
                    think_ms: None,
                }
            })
            .collect();
        GameRecord {
            game_id: "t".into(),
            context: GameContext::Pvp { wager_sol: 0.0 },
            white: PlayerRef {
                pubkey: "w".into(),
                elo: 1500,
            },
            black: PlayerRef {
                pubkey: "b".into(),
                elo: 1500,
            },
            time_control: TimeControl {
                base_sec: 600,
                inc_sec: 0,
            },
            start_fen: String::new(),
            moves,
            result: GameResult::Draw,
            ended_at_ms: 0,
        }
    }

    #[test]
    fn metronome_play_is_suspicious() {
        // White moves every ~8s like clockwork and never snaps; black varies.
        let g = game(
            60,
            &[7_900, 8_000, 8_100],
            &[500, 3_000, 12_000, 25_000, 1_000],
        );
        let r = t0_screen(&g);
        assert!(r.white_score >= SUSPICIOUS_SCORE, "white {}", r.white_score);
        assert!(r.black_score < SUSPICIOUS_SCORE, "black {}", r.black_score);
        assert!(r.suspicious);
    }

    #[test]
    fn human_variance_is_clean() {
        let human = &[800, 2_500, 15_000, 4_000, 600, 30_000, 9_000, 1_200];
        let g = game(60, human, human);
        let r = t0_screen(&g);
        assert!(
            !r.suspicious,
            "white {} black {}",
            r.white_score, r.black_score
        );
    }

    #[test]
    fn short_games_abstain() {
        let g = game(16, &[8_000], &[8_000]);
        let r = t0_screen(&g);
        assert_eq!(r.white_score, 0.0);
        assert!(!r.suspicious);
    }

    #[test]
    fn fast_uniform_bullet_is_not_flagged() {
        // Uniformly fast play is normal blitz behavior, not metronoming.
        let g = game(60, &[900, 1_000, 1_100], &[900, 1_000, 1_100]);
        let r = t0_screen(&g);
        assert!(
            !r.suspicious,
            "white {} black {}",
            r.white_score, r.black_score
        );
    }

    #[test]
    fn heavy_blur_escalates_even_with_human_timing() {
        let human = &[800, 2_500, 15_000, 4_000, 600, 30_000, 9_000, 1_200];
        let mut g = game(60, human, human);
        // White's client reports blur on nearly every move.
        for (i, m) in g.moves.iter_mut().enumerate() {
            if i % 2 == 0 {
                m.blurred = true;
            }
        }
        let r = t0_screen(&g);
        assert!(r.white_score >= SUSPICIOUS_SCORE, "white {}", r.white_score);
        assert!(r.suspicious);
    }

    #[test]
    fn batch_collapsed_timing_does_not_false_positive() {
        // Metronome latencies, but all timestamps within a few ms of each
        // other (GameEndBatch). Timing source resolves to None, so the
        // flat-time/no-snap heuristics must NOT fire.
        let mut g = game(60, &[8_000, 8_000, 8_000], &[8_000, 8_000, 8_000]);
        let base = 1_700_000_000_000u64;
        for (i, m) in g.moves.iter_mut().enumerate() {
            m.signed_at_ms = base + i as u64 * 5; // ~5ms apart: implausible
        }
        let r = t0_screen(&g);
        assert!(
            !r.suspicious,
            "white {} black {}",
            r.white_score, r.black_score
        );
    }

    #[test]
    fn client_think_times_drive_screen_when_batched() {
        // Same batched timestamps, but white reports flat 8s think times.
        // The screen should catch white via the client source and clear black.
        let mut g = game(60, &[8_000, 8_000, 8_000], &[8_000, 8_000, 8_000]);
        let base = 1_700_000_000_000u64;
        for (i, m) in g.moves.iter_mut().enumerate() {
            m.signed_at_ms = base + i as u64 * 5;
            if i % 2 == 0 {
                m.think_ms = Some(8_000); // white: metronome think times
            }
        }
        let r = t0_screen(&g);
        assert!(r.white_score >= SUSPICIOUS_SCORE, "white {}", r.white_score);
        assert_eq!(r.black_score, 0.0, "black has no trusted timing");
    }
}
