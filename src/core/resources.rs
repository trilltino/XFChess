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

    /// Whether audio is muted
    #[serde(default)]
    pub muted: bool,

    /// Whether to show move hints
    pub show_hints: bool,

    /// Whether to highlight last move
    pub highlight_last_move: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.7,
            muted: false,
            show_hints: true,
            highlight_last_move: true,
        }
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
