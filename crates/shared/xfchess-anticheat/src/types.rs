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
    pub signed_at_ms: u64,
    pub latency_ms: u32,
    #[serde(default)]
    pub blurred: bool,
    #[serde(default)]
    pub think_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimingSource {
    Client,
    Server,
    #[default]
    None,
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
    pub cpl: i32,
    pub is_t1: bool,
    pub complexity: Complexity,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Complexity {
    Forced,
    Simple,
    Complex,
}

// ── Per-side signal values ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalValues {
    pub timing_anomaly: f64,
    pub cpl_vs_elo: f64,
    pub t1_rate: f64,
    pub avg_cpl: f64,
    pub complex_ply_count: u32,
    #[serde(default)]
    pub blur_rate: f64,
    #[serde(default)]
    pub timing_source: TimingSource,
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
