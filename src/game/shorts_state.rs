use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStyle {
    TopBold,
    BottomCaption,
    CenterDramatic,
}

#[derive(Debug, Clone)]
pub struct HookText {
    pub text: String,
    pub style: HookStyle,
}

#[derive(Debug, Clone)]
pub struct CaptureSequence {
    pub from_ply: usize,
    pub to_ply: usize,
    pub current: usize,
    pub delay_secs: f32,
    pub timer: f32,
    pub output_dir: std::path::PathBuf,
}

#[derive(Resource, Default)]
pub struct ShortsState {
    pub content_tier: ContentTier,
    pub hook_texts: HashMap<usize, HookText>,
    pub beat_markers: BTreeMap<usize, String>,
    pub capture_mode: Option<CaptureSequence>,
    pub hook_text_alpha: f32,
    pub show_hook_editor: bool,
    pub hook_input: String,
    pub show_beat_export: bool,
    pub capture_from_input: String,
    pub capture_to_input: String,
}
