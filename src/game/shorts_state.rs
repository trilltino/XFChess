use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap};

/// Content tier for the chess-shorts workflow — each tier applies a different
/// cinematic preset when the creator loads a PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentTier {
    #[default]
    None,
    Puzzle,
    Blunder,
    Highlight,
    OpeningTrap,
}

impl ContentTier {
    pub fn label(self) -> &'static str {
        match self {
            ContentTier::None => "None",
            ContentTier::Puzzle => "🧩 Puzzle",
            ContentTier::Blunder => "⚡ Blunder",
            ContentTier::Highlight => "🏆 Highlight",
            ContentTier::OpeningTrap => "🎣 Opening Trap",
        }
    }
    pub fn default_hook(self) -> &'static str {
        match self {
            ContentTier::Puzzle => "White to move — can you find it?",
            ContentTier::Blunder => "This move lost the game.",
            ContentTier::Highlight => "The move that changed everything.",
            ContentTier::OpeningTrap => "This trick wins in 4 moves every time.",
            ContentTier::None => "",
        }
    }
}

/// Visual style for the hook text overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStyle {
    /// Large bold text at the top of the screen — "99% of players miss this…"
    TopBold,
    /// Subtitle-style caption at the bottom
    BottomCaption,
    /// Large dramatic text dead-centre with dark background
    CenterDramatic,
}

/// A creator-authored text overlay tied to a specific ply.
#[derive(Debug, Clone)]
pub struct HookText {
    pub text: String,
    pub style: HookStyle,
}

/// State for the auto-capture-sequence mode (screenshot every ply).
#[derive(Debug, Clone)]
pub struct CaptureSequence {
    pub from_ply: usize,
    pub to_ply: usize,
    pub current: usize,
    /// Seconds to wait after ply change before capturing (lets tweens settle)
    pub delay_secs: f32,
    pub timer: f32,
    pub output_dir: std::path::PathBuf,
}

/// Global resource holding all content-creation state for the shorts workflow.
#[derive(Resource, Default)]
pub struct ShortsState {
    /// Selected content tier — drives cinematic presets
    pub content_tier: ContentTier,
    /// Creator-authored hook texts keyed by ply index
    pub hook_texts: HashMap<usize, HookText>,
    /// Audio beat markers keyed by ply; value is the beat label
    pub beat_markers: BTreeMap<usize, String>,
    /// Active sequence capture session (None = not capturing)
    pub capture_mode: Option<CaptureSequence>,
    /// Current hook text fade alpha (0.0 = invisible, 1.0 = fully visible)
    pub hook_text_alpha: f32,
    /// Whether the hook text editor panel is open
    pub show_hook_editor: bool,
    /// Text input buffer for the hook text editor
    pub hook_input: String,
    /// Whether the beat-marker export panel is open
    pub show_beat_export: bool,
    /// Capture UI: from ply input buffer
    pub capture_from_input: String,
    /// Capture UI: to ply input buffer
    pub capture_to_input: String,
}
