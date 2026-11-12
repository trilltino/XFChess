use crate::elo_baseline::cpl_vs_elo_signal;
use crate::types::{Complexity, PlyEval};

pub fn avg_cpl(plies: &[PlyEval]) -> f64 {
    let (sum, count) = plies
        .iter()
        .filter(|ply| ply.complexity != Complexity::Forced)
        .fold((0.0, 0usize), |(sum, count), ply| {
            (sum + f64::from(ply.cpl), count + 1)
        });
    if count == 0 {
        return 0.0;
    }
    sum / count as f64
}

pub fn t1_rate(plies: &[PlyEval]) -> f64 {
    let (top, count) = plies
        .iter()
        .filter(|ply| ply.complexity == Complexity::Complex)
        .fold((0usize, 0usize), |(top, count), ply| {
            (top + usize::from(ply.is_t1), count + 1)
        });
    if count == 0 {
        return 0.0;
    }
    top as f64 / count as f64
}

pub fn cpl_signal(elo: u32, plies: &[PlyEval]) -> f64 {
    let observed = avg_cpl(plies);
    cpl_vs_elo_signal(elo, observed)
}

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
