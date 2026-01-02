//! Core resources for game-wide state management
//!
//! These resources are used across multiple states and provide global
//! configuration and tracking capabilities.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Resource tracking settings that can be changed from the settings menu
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Reflect)]
#[reflect(Resource)]
pub struct GameSettings {
    /// Master volume (0.0 to 1.0)
    pub master_volume: f32,

    /// Graphics quality preset
    pub graphics_quality: GraphicsQuality,

    /// Whether to show move hints
    pub show_hints: bool,

    /// Whether to highlight last move
    pub highlight_last_move: bool,

    /// Board theme
    pub board_theme: BoardTheme,

    /// Dynamic orbital lighting settings
    pub dynamic_lighting: DynamicLightingSettings,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.7,
            graphics_quality: GraphicsQuality::High,
            show_hints: true,
            highlight_last_move: true,
            board_theme: BoardTheme::Classic,
            dynamic_lighting: DynamicLightingSettings::default(),
        }
    }
}

/// Graphics quality presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum GraphicsQuality {
    Low,
    Medium,
    High,
    Ultra,
}

impl GraphicsQuality {
    pub fn description(&self) -> &'static str {
        match self {
            GraphicsQuality::Low => "Low (Better Performance)",
            GraphicsQuality::Medium => "Medium (Balanced)",
            GraphicsQuality::High => "High (Better Quality)",
            GraphicsQuality::Ultra => "Ultra (Best Quality)",
        }
    }

    pub fn shadow_enabled(&self) -> bool {
        !matches!(self, GraphicsQuality::Low)
    }

    pub fn bloom_enabled(&self) -> bool {
        matches!(self, GraphicsQuality::High | GraphicsQuality::Ultra)
    }

    pub fn ambient_occlusion_enabled(&self) -> bool {
        matches!(self, GraphicsQuality::Ultra)
    }
}

/// Board visual themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum BoardTheme {
    Classic,
    Modern,
    Wood,
    Marble,
}

impl BoardTheme {
    pub fn name(&self) -> &'static str {
        match self {
            BoardTheme::Classic => "Classic",
            BoardTheme::Modern => "Modern",
            BoardTheme::Wood => "Wood",
            BoardTheme::Marble => "Marble",
        }
    }

    /// Returns (light_square_color, dark_square_color)
    pub fn colors(&self) -> (Color, Color) {
        match self {
            BoardTheme::Classic => (
                Color::srgb(0.93, 0.93, 0.82), // Cream
                Color::srgb(0.46, 0.59, 0.34), // Green
            ),
            BoardTheme::Modern => (
                Color::srgb(0.9, 0.9, 0.9), // Light gray
                Color::srgb(0.3, 0.3, 0.3), // Dark gray
            ),
            BoardTheme::Wood => (
                Color::srgb(0.85, 0.70, 0.55), // Light wood
                Color::srgb(0.55, 0.35, 0.20), // Dark wood
            ),
            BoardTheme::Marble => (
                Color::srgb(0.95, 0.95, 0.95), // White marble
                Color::srgb(0.15, 0.15, 0.15), // Black marble
            ),
        }
    }
}

/// Dynamic orbital lighting configuration
#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct DynamicLightingSettings {
    /// Whether dynamic orbital lighting is enabled
    pub enabled: bool,

    /// Number of orbital lights (2-6)
    pub light_count: u32,

    /// Colors for each light (stored as sRGB tuples for serialization)
    #[serde(skip)]
    pub light_colors: Vec<Color>,

    /// Serialized colors for persistence (internal use)
    #[serde(default, rename = "light_colors")]
    light_colors_serialized: Vec<(f32, f32, f32)>,

    /// Radius of orbital path around board center
    pub orbital_radius: f32,

    /// Rotation speed (radians per second)
    pub orbital_speed: f32,

    /// Height of lights above board
    pub orbital_height: f32,

    /// Whether lights cast shadows
    pub shadows_enabled: bool,
}

impl Default for DynamicLightingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            light_count: 3,
            light_colors: vec![
                Color::srgb(1.0, 0.3, 0.3), // Red
                Color::srgb(0.3, 1.0, 0.3), // Green
                Color::srgb(0.3, 0.3, 1.0), // Blue
            ],
            light_colors_serialized: vec![(1.0, 0.3, 0.3), (0.3, 1.0, 0.3), (0.3, 0.3, 1.0)],
            orbital_radius: 12.0,
            orbital_speed: 0.5,
            orbital_height: 10.0,
            shadows_enabled: true,
        }
    }
}

impl DynamicLightingSettings {
    /// Get color for a specific light index, cycling through available colors
    pub fn get_color(&self, index: usize) -> Color {
        if self.light_colors.is_empty() {
            Color::WHITE
        } else {
            self.light_colors[index % self.light_colors.len()]
        }
    }

    /// Update light colors when light count changes
    pub fn update_colors_for_count(&mut self) {
        // Ensure we have enough colors for the current light count
        while self.light_colors.len() < self.light_count as usize {
            // Add default colors if needed
            let default_colors = [
                Color::srgb(1.0, 0.3, 0.3), // Red
                Color::srgb(0.3, 1.0, 0.3), // Green
                Color::srgb(0.3, 0.3, 1.0), // Blue
                Color::srgb(1.0, 1.0, 0.3), // Yellow
                Color::srgb(1.0, 0.3, 1.0), // Magenta
                Color::srgb(0.3, 1.0, 1.0), // Cyan
            ];
            let color_index = self.light_colors.len() % default_colors.len();
            self.light_colors.push(default_colors[color_index]);
            self.light_colors_serialized.push({
                let srgba = default_colors[color_index].to_srgba();
                (srgba.red, srgba.green, srgba.blue)
            });
        }
        // Trim excess colors if count decreased
        self.light_colors.truncate(self.light_count as usize);
        self.light_colors_serialized
            .truncate(self.light_count as usize);
    }

    /// Sync serialized colors from Color vec (call before serialization)
    pub fn sync_for_serialization(&mut self) {
        self.light_colors_serialized = self
            .light_colors
            .iter()
            .map(|c| {
                let srgba = c.to_srgba();
                (srgba.red, srgba.green, srgba.blue)
            })
            .collect();
    }

    /// Sync Color vec from serialized colors (call after deserialization)
    pub fn sync_from_serialized(&mut self) {
        self.light_colors = self
            .light_colors_serialized
            .iter()
            .map(|(r, g, b)| Color::srgb(*r, *g, *b))
            .collect();
    }
}

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

    pub fn average_moves(&self) -> f32 {
        if self.games_played > 0 {
            self.total_moves as f32 / self.games_played as f32
        } else {
            0.0
        }
    }

    pub fn win_rate_white(&self) -> f32 {
        if self.games_played > 0 {
            self.white_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }

    pub fn win_rate_black(&self) -> f32 {
        if self.games_played > 0 {
            self.black_wins as f32 / self.games_played as f32 * 100.0
        } else {
            0.0
        }
    }
}
