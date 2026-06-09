use crate::config::AcConfig;
use crate::types::Complexity;

/// Classify a position by how much choice the player had.
pub fn classify(top1_cp: i32, top2_cp: i32, cfg: &AcConfig) -> Complexity {
    let delta = (top1_cp - top2_cp).abs();
    if delta < cfg.forced_delta_cp {
        Complexity::Forced
    } else if delta < cfg.complex_delta_cp {
        Complexity::Simple
    } else {
        Complexity::Complex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AcConfig;

    fn cfg() -> AcConfig { AcConfig::default() }

    #[test] fn forced() { assert_eq!(classify(100, 95, &cfg()), Complexity::Forced); }
    #[test] fn simple() { assert_eq!(classify(100, 65, &cfg()), Complexity::Simple); }
    #[test] fn complex() { assert_eq!(classify(100, 40, &cfg()), Complexity::Complex); }
}
