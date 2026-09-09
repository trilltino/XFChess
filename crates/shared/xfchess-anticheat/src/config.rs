#[derive(Debug, Clone)]
pub struct AcConfig {
    // ── Stockfish ──────────────────────────────────────────────────────────────
    pub stockfish_path: String,
    pub analysis_depth: u8,
    pub movetime_ms: u64,
    pub worker_count: usize,

    // ── Complexity filter ──────────────────────────────────────────────────────
    pub forced_delta_cp: i32,
    pub complex_delta_cp: i32,

    // ── Timing signal ──────────────────────────────────────────────────────────
    pub timing_fast_threshold_ms: u32,
    pub timing_disable_below_sec: u32,

    // ── Scoring thresholds ─────────────────────────────────────────────────────
    pub review_threshold: f64,
    pub flag_threshold: f64,

    // ── Signal weights (must sum to 1.0) ──────────────────────────────────────
    pub weight_timing: f64,
    pub weight_cpl_vs_elo: f64,
    pub weight_t1_rate: f64,
    pub weight_blur: f64,

    // ── Minimum samples ───────────────────────────────────────────────────────
    pub min_complex_plies: u32,

    // ── Reports ───────────────────────────────────────────────────────────────
    pub reports_dir: String,

    // ── Job queue ──────────────────────────────────────────────────────────────
    pub queue_capacity: usize,
    pub max_attempts: u32,
}

impl Default for AcConfig {
    fn default() -> Self {
        Self {
            stockfish_path: "stockfish".into(),
            analysis_depth: 18,
            movetime_ms: 2_000,
            worker_count: 2,

            forced_delta_cp: 20,
            complex_delta_cp: 50,

            timing_fast_threshold_ms: 2_000,
            timing_disable_below_sec: 30,

            review_threshold: 0.60,
            flag_threshold: 0.80,

            weight_timing: 0.40,
            weight_cpl_vs_elo: 0.35,
            weight_t1_rate: 0.25,
            weight_blur: 0.15,

            min_complex_plies: 5,

            reports_dir: "reports".into(),
            queue_capacity: 512,
            max_attempts: 3,
        }
    }
}

impl AcConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(p) = std::env::var("STOCKFISH_PATH") {
            cfg.stockfish_path = p;
        }
        if let Ok(d) = std::env::var("AC_DEPTH")
            .and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent))
        {
            cfg.analysis_depth = d;
        }
        if let Ok(w) = std::env::var("AC_WORKERS")
            .and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent))
        {
            cfg.worker_count = w;
        }
        if let Ok(r) = std::env::var("AC_REPORTS_DIR") {
            cfg.reports_dir = r;
        }
        cfg
    }
}
