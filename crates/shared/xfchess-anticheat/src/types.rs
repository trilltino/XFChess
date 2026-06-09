use serde::{Deserialize, Serialize};

// ── Input types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    pub game_id: String,
    pub context: GameContext,
    pub white: PlayerRef,
    pub black: PlayerRef,
    pub time_control: TimeControl,
    pub start_fen: String,
    pub moves: Vec<MoveRecord>,
    pub result: GameResult,
    pub ended_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameContext {
    Pvp { wager_sol: f64 },
    Tournament { tournament_id: u64, round: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRef {
    pub pubkey: String,
    pub elo: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeControl {
    pub base_sec: u32,
    pub inc_sec: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRecord {
    pub ply: u32,
    pub move_uci: String,
    pub fen_after: String,
    /// Server wall-clock ms when move was signed.
    pub signed_at_ms: u64,
    /// signed_at_ms - prev signed_at_ms (0 for ply 0).
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameResult {
    WhiteWin,
    BlackWin,
    Draw,
}

// ── Per-ply evaluation ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlyEval {
    pub ply: u32,
    pub move_uci: String,
    pub top1_cp: i32,
    pub top2_cp: i32,
    /// Centipawn loss: top1_cp − played_cp (clamped to 0 minimum).
    pub cpl: i32,
    pub is_t1: bool,
    pub complexity: Complexity,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Complexity {
    /// top1 − top2 < forced_delta_cp: only reasonable move.
    Forced,
    /// top1 − top2 in [forced_delta, complex_delta): clear best but not only.
    Simple,
    /// top1 − top2 >= complex_delta: multiple moves are plausible.
    Complex,
}

// ── Per-side signal values ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalValues {
    /// Ratio of suspiciously fast moves on Complex positions (0.0–1.0).
    pub timing_anomaly: f64,
    /// Sigmoid-mapped z-score: how far below ELO-expected CPL the player is (0.0–1.0).
    pub cpl_vs_elo: f64,
    /// T1 rate on Complex positions only (0.0–1.0).
    pub t1_rate: f64,
    /// Average CPL across non-Forced plies (for display).
    pub avg_cpl: f64,
    /// Number of Complex positions used in the sample.
    pub complex_ply_count: u32,
}

// ── Verdict ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Verdict {
    Clean,
    Review,
    Flag,
}

impl Verdict {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.80 {
            Verdict::Flag
        } else if score >= 0.60 {
            Verdict::Review
        } else {
            Verdict::Clean
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Clean => "Clean",
            Verdict::Review => "Review",
            Verdict::Flag => "Flag",
        }
    }
}

// ── Full analysis output ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideAnalysis {
    pub pubkey: String,
    pub elo: u32,
    pub signals: SignalValues,
    pub weighted_score: f64,
    pub verdict: Verdict,
    pub ply_evals: Vec<PlyEval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcReport {
    pub game_id: String,
    pub context: GameContext,
    pub result: GameResult,
    pub white: SideAnalysis,
    pub black: SideAnalysis,
    pub engine_version: String,
    pub analysis_depth: u8,
    pub analysed_at_ms: u64,
}
