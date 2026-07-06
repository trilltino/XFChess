use crate::elo_baseline::cpl_vs_elo_signal;
use crate::types::{Complexity, PlyEval};

/// Average CPL across non-Forced plies.
pub fn avg_cpl(plies: &[PlyEval]) -> f64 {
    let relevant: Vec<_> = plies
        .iter()
        .filter(|p| p.complexity != Complexity::Forced)
        .collect();
    if relevant.is_empty() {
        return 0.0;
    }
    let sum: i32 = relevant.iter().map(|p| p.cpl).sum();
    sum as f64 / relevant.len() as f64
}

/// T1 rate on Complex positions only.
pub fn t1_rate(plies: &[PlyEval]) -> f64 {
    let complex: Vec<_> = plies
        .iter()
        .filter(|p| p.complexity == Complexity::Complex)
        .collect();
    if complex.is_empty() {
        return 0.0;
    }
    let t1_count = complex.iter().filter(|p| p.is_t1).count();
    t1_count as f64 / complex.len() as f64
}

/// CPL-vs-ELO signal: how suspicious is this player's accuracy for their rating?
pub fn cpl_signal(elo: u32, plies: &[PlyEval]) -> f64 {
    let observed = avg_cpl(plies);
    cpl_vs_elo_signal(elo, observed)
}

/// Count of Complex positions (minimum sample gate).
pub fn complex_ply_count(plies: &[PlyEval]) -> u32 {
    plies
        .iter()
        .filter(|p| p.complexity == Complexity::Complex)
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Complexity, PlyEval};

    fn make_ply(cpl: i32, is_t1: bool, complexity: Complexity) -> PlyEval {
        PlyEval {
            ply: 0,
            move_uci: "e2e4".into(),
            top1_cp: 100,
            top2_cp: 0,
            cpl,
            is_t1,
            complexity,
            latency_ms: 3000,
        }
    }

    #[test]
    fn avg_cpl_excludes_forced() {
        let plies = vec![
            make_ply(0, true, Complexity::Forced),
            make_ply(20, false, Complexity::Complex),
            make_ply(40, false, Complexity::Simple),
        ];
        let cpl = avg_cpl(&plies);
        assert!((cpl - 30.0).abs() < 0.1);
    }

    #[test]
    fn t1_rate_complex_only() {
        let plies = vec![
            make_ply(0, true, Complexity::Complex),
            make_ply(0, false, Complexity::Complex),
            make_ply(0, true, Complexity::Simple), // should be ignored
        ];
        let rate = t1_rate(&plies);
        assert!((rate - 0.5).abs() < 0.01);
    }
}
