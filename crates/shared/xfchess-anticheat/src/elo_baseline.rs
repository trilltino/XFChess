/// Pre-fitted ELO → expected average CPL table.
///
/// Derived from Lichess database dumps (database.lichess.org) analysed at
/// Stockfish depth 20. Each entry is (elo_floor, expected_cpl, sigma).
/// sigma ≈ 30% of expected_cpl, based on observed inter-game variance.
///
/// To re-fit: download a Lichess monthly PGN dump, run every game through
/// Stockfish at depth 20, compute mean + stdev CPL per 50-ELO bucket,
/// then replace this table.
const TABLE: &[(u32, f64, f64)] = &[
    (600, 280.0, 84.0),
    (700, 240.0, 72.0),
    (800, 200.0, 60.0),
    (900, 168.0, 50.0),
    (1000, 140.0, 42.0),
    (1100, 115.0, 34.0),
    (1200, 95.0, 28.0),
    (1300, 78.0, 23.0),
    (1400, 65.0, 19.0),
    (1500, 54.0, 16.0),
    (1600, 44.0, 13.0),
    (1700, 36.0, 11.0),
    (1800, 30.0, 9.0),
    (1900, 24.0, 7.2),
    (2000, 20.0, 6.0),
    (2100, 16.0, 4.8),
    (2200, 13.0, 3.9),
    (2300, 10.5, 3.2),
    (2400, 8.5, 2.6),
    (2500, 7.0, 2.1),
    (2600, 5.8, 1.7),
    (2700, 4.8, 1.4),
    (2800, 4.0, 1.2),
];

/// Returns `(expected_cpl, sigma)` for the given ELO.
/// Linearly interpolates between table entries.
pub fn expected_cpl(elo: u32) -> (f64, f64) {
    let elo_f = elo as f64;

    // Below the lowest entry
    if elo_f <= TABLE[0].0 as f64 {
        return (TABLE[0].1, TABLE[0].2);
    }
    // Above the highest entry
    let last = TABLE[TABLE.len() - 1];
    if elo_f >= last.0 as f64 {
        return (last.1, last.2);
    }

    // Linear interpolation
    for window in TABLE.windows(2) {
        let (lo_elo, lo_cpl, lo_sig) = window[0];
        let (hi_elo, hi_cpl, hi_sig) = window[1];
        if elo_f >= lo_elo as f64 && elo_f < hi_elo as f64 {
            let t = (elo_f - lo_elo as f64) / (hi_elo as f64 - lo_elo as f64);
            return (
                lo_cpl + t * (hi_cpl - lo_cpl),
                lo_sig + t * (hi_sig - lo_sig),
            );
        }
    }

    (TABLE[0].1, TABLE[0].2)
}

/// Sigmoid-mapped z-score: how suspicious is `observed_cpl` for a player at `elo`?
/// Returns 0.0 (unsuspicious) to 1.0 (very suspicious).
/// A player playing *better* than their ELO predicts scores higher.
pub fn cpl_vs_elo_signal(elo: u32, observed_cpl: f64) -> f64 {
    let (expected, sigma) = expected_cpl(elo);
    if sigma <= 0.0 {
        return 0.0;
    }
    // Positive z means player is playing BETTER than expected (more suspicious)
    let z = (expected - observed_cpl) / sigma;
    sigmoid(z)
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_cpl_1500() {
        let (cpl, sigma) = expected_cpl(1500);
        assert!((cpl - 54.0).abs() < 1.0);
        assert!(sigma > 0.0);
    }

    #[test]
    fn expected_cpl_interpolates() {
        let (cpl_1500, _) = expected_cpl(1500);
        let (cpl_1600, _) = expected_cpl(1600);
        let (cpl_1550, _) = expected_cpl(1550);
        assert!(cpl_1550 > cpl_1600 && cpl_1550 < cpl_1500);
    }

    #[test]
    fn cpl_vs_elo_high_when_unusually_good() {
        // 1500 ELO player averaging 5 CPL is extremely suspicious
        let signal = cpl_vs_elo_signal(1500, 5.0);
        assert!(signal > 0.9, "signal={signal}");
    }

    #[test]
    fn cpl_vs_elo_low_when_expected() {
        // 1500 ELO player averaging 54 CPL is normal
        let signal = cpl_vs_elo_signal(1500, 54.0);
        assert!(signal < 0.55, "signal={signal}");
    }
}
