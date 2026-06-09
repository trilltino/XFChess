use crate::config::AcConfig;
use crate::types::{SignalValues, Verdict};

/// Compute a weighted score in [0.0, 1.0] and derive the verdict.
pub fn score(signals: &SignalValues, cfg: &AcConfig) -> (f64, Verdict) {
    let s = cfg.weight_timing * signals.timing_anomaly
        + cfg.weight_cpl_vs_elo * signals.cpl_vs_elo
        + cfg.weight_t1_rate * signals.t1_rate;
    let clamped = s.clamp(0.0, 1.0);
    let verdict = Verdict::from_score(clamped);
    (clamped, verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AcConfig;
    use crate::types::{SignalValues, Verdict};

    fn signals(t: f64, c: f64, r: f64) -> SignalValues {
        SignalValues {
            timing_anomaly: t,
            cpl_vs_elo: c,
            t1_rate: r,
            avg_cpl: 10.0,
            complex_ply_count: 10,
        }
    }

    #[test]
    fn clean_when_all_zero() {
        let (s, v) = score(&signals(0.0, 0.0, 0.0), &AcConfig::default());
        assert!(s < 0.60);
        assert_eq!(v, Verdict::Clean);
    }

    #[test]
    fn flag_when_all_high() {
        let (s, v) = score(&signals(1.0, 1.0, 1.0), &AcConfig::default());
        assert!(s >= 0.80);
        assert_eq!(v, Verdict::Flag);
    }

    #[test]
    fn review_at_midpoint() {
        let (_, v) = score(&signals(0.7, 0.7, 0.7), &AcConfig::default());
        assert_eq!(v, Verdict::Review);
    }
}
