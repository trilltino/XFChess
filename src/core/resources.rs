//! Core resources for game-wide state management
//!
//! These resources are used across multiple states and provide global
//! configuration and tracking capabilities.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Graphics quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Reflect)]
pub enum GraphicsQuality {
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl GraphicsQuality {
    pub fn bloom_enabled(self) -> bool {
        matches!(self, Self::High | Self::Ultra)
    }

    pub fn ambient_occlusion_enabled(self) -> bool {
        self == Self::Ultra
    }

    pub fn shadow_enabled(self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Shadow atlas resolution in texels (used for `ShadowmapSettings`).
    pub fn shadow_map_size(self) -> u32 {
        match self {
            Self::Low => 512,
            Self::Medium => 1024,
            Self::High => 2048,
            Self::Ultra => 4096,
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "Low – shadows off, no bloom, fastest",
            Self::Medium => "Medium – shadows on, no bloom",
            Self::High => "High – shadows + bloom",
            Self::Ultra => "Ultra – shadows + bloom + SSAO",
        }
    }
}

/// Dynamic orbital lighting configuration
#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct DynamicLightingSettings {
    pub enabled: bool,
    pub light_count: u32,
    pub orbital_radius: f32,
    pub orbital_height: f32,
    pub orbital_speed: f32,
    pub shadows_enabled: bool,
    /// Custom colors (RGB 0-1) per light slot; falls back to defaults when empty.
    #[serde(default)]
    pub colors: Vec<[f32; 3]>,
}

impl DynamicLightingSettings {
    pub fn get_color(&self, index: usize) -> bevy::prelude::Color {
        const DEFAULTS: [[f32; 3]; 6] = [
            [1.0, 0.95, 0.85],
            [0.95, 1.0, 0.88],
            [1.0, 0.92, 0.80],
            [0.90, 0.95, 1.0],
            [1.0, 0.95, 0.85],
            [0.95, 1.0, 0.88],
        ];
        let rgb = self.colors.get(index).copied()
            .unwrap_or(DEFAULTS[index % DEFAULTS.len()]);
        bevy::prelude::Color::srgb(rgb[0], rgb[1], rgb[2])
    }

    /// Max lights allowed for a given quality tier.
    pub fn quality_cap(quality: GraphicsQuality) -> u32 {
        match quality {
            GraphicsQuality::Low => 2,
            GraphicsQuality::Medium => 3,
            GraphicsQuality::High => 4,
            GraphicsQuality::Ultra => 6,
        }
    }
}

impl Default for DynamicLightingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            light_count: 4,
            orbital_radius: 6.0,
            orbital_height: 4.0,
            orbital_speed: 0.3,
            shadows_enabled: false,
            colors: Vec::new(),
        }
    }
}

/// Resource tracking settings that can be changed from the settings menu
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Reflect)]
#[reflect(Resource)]
pub struct GameSettings {
    /// Master volume (0.0 to 1.0)
    pub master_volume: f32,

    /// Whether audio is muted
    #[serde(default)]
    pub muted: bool,

    /// Whether to show move hints
    pub show_hints: bool,

    /// Whether to highlight last move
    pub highlight_last_move: bool,

    /// Whether to use VPS relay for P2P connections
    #[serde(default = "default_true")]
    pub use_vps_relay: bool,

    /// Graphics quality preset
    #[serde(default)]
    pub graphics_quality: GraphicsQuality,

    /// Dynamic orbital lighting configuration
    #[serde(default)]
    pub dynamic_lighting: DynamicLightingSettings,

    /// 2D board colour theme index (0=Classic, 1=Green, 2=Blue, 3=Purple, 4=Dark)
    #[serde(default)]
    pub board_theme: u8,

    /// Blindfold mode — hides piece symbols on the 2D board
    #[serde(default)]
    pub blindfold: bool,

    /// 2D piece set index (0=CBurnett, 1=Alpha, 2=Merida)
    #[serde(default)]
    pub piece_set: u8,

    /// Show vertical centipawn eval bar (offline/AI games only)
    #[serde(default)]
    pub show_eval_bar: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.7,
            muted: false,
            show_hints: true,
            highlight_last_move: true,
            use_vps_relay: true,
            graphics_quality: GraphicsQuality::Medium,
            dynamic_lighting: DynamicLightingSettings::default(),
            board_theme: 0,
            blindfold: false,
            piece_set: 0,
            show_eval_bar: false,
        }
    }
}

fn default_true() -> bool { true }

/// Resource for tracking game statistics
#[derive(Resource, Debug, Clone, Default, Reflect)]
#[reflect(Resource)]
pub struct GameStatistics {
    /// Total games played
    pub games_played: u32,

    /// Games won as white
    pub white_wins: u32,

    /// Games won as black
    pub black_wins: u32,

    /// Draws
    pub draws: u32,

    /// Total moves made
    pub total_moves: u32,

    /// Longest game (in moves)
    pub longest_game: u32,

    /// Shortest game (in moves)
    pub shortest_game: u32,
}

impl GameStatistics {
    pub fn record_game(
        &mut self,
        winner: Option<crate::rendering::pieces::PieceColor>,
        moves: u32,
    ) {
        use crate::rendering::pieces::PieceColor;

        self.games_played += 1;
        self.total_moves += moves;

        match winner {
            Some(PieceColor::White) => self.white_wins += 1,
            Some(PieceColor::Black) => self.black_wins += 1,
            None => self.draws += 1,
        }

        if self.games_played == 1 {
            self.longest_game = moves;
            self.shortest_game = moves;
        } else {
            self.longest_game = self.longest_game.max(moves);
            self.shortest_game = self.shortest_game.min(moves);
        }
    }

    #[allow(dead_code)]
    pub fn average_moves(&self) -> f32 {
        if self.games_played > 0 {
            self.total_moves as f32 / self.games_played as f32
        } else {
            0.0
        }
    }

    #[allow(dead_code)]
    pub fn win_rate_white(&self) -> f32 {
        if self.games_played > 0 {
            self.white_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }

    #[allow(dead_code)]
    pub fn win_rate_black(&self) -> f32 {
        if self.games_played > 0 {
            self.black_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }
}
