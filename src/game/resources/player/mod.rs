//! Player interaction resources
//!
//! Manages player information and piece selection state.

use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

pub mod promotion;
pub mod selection;
pub use promotion::*;
pub use selection::*;

/// Individual player information
///
/// Represents a single player in the game with their identity,
/// color, and control type (human or AI).
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Player {
    /// Player ID (1 or 2)
    pub id: u8,

    /// Player name (e.g., "Player 1", "Player 2", "AI")
    pub name: String,

    /// Player's piece color (White or Black)
    pub color: PieceColor,

    /// Whether this player is human-controlled (false = AI)
    pub is_human: bool,
}

impl Player {
    /// Create a new player
    pub fn new(id: u8, name: String, color: PieceColor, is_human: bool) -> Self {
        Self {
            id,
            name,
            color,
            is_human,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Player".to_string(),
            color: PieceColor::White,
            is_human: true,
        }
    }
}

/// Container resource holding both players
///
/// Provides easy access to player information based on color or ID.
#[derive(Resource, Debug, Reflect, Default)]
#[reflect(Resource)]
pub struct Players {
    /// Player 1 (typically White)
    pub player_1: Player,

    /// Player 2 (typically Black)
    pub player_2: Player,
}

impl Players {
    /// Get player by their piece color
    #[allow(dead_code)] // Public API - useful for UI and game logic
    pub fn by_color(&self, color: PieceColor) -> &Player {
        match color {
            PieceColor::White => &self.player_1,
            PieceColor::Black => &self.player_2,
        }
    }

    /// Get player by ID (1 or 2)
    #[allow(dead_code)] // Public API - useful for UI and game logic
    pub fn by_id(&self, id: u8) -> Option<&Player> {
        match id {
            1 => Some(&self.player_1),
            2 => Some(&self.player_2),
            _ => None,
        }
    }

    /// Get the current player based on turn color
    #[allow(dead_code)] // Public API - useful for UI and game logic
    pub fn current(&self, current_color: PieceColor) -> &Player {
        self.by_color(current_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let player = Player::new(1, "Player 1".to_string(), PieceColor::White, true);
        assert_eq!(player.id, 1);
        assert_eq!(player.name, "Player 1");
        assert_eq!(player.color, PieceColor::White);
        assert!(player.is_human);
    }

    #[test]
    fn test_players_by_color() {
        let players = Players {
            player_1: Player::new(1, "White".to_string(), PieceColor::White, true),
            player_2: Player::new(2, "Black".to_string(), PieceColor::Black, true),
        };

        assert_eq!(players.by_color(PieceColor::White).id, 1);
        assert_eq!(players.by_color(PieceColor::Black).id, 2);
    }

    #[test]
    fn test_players_current() {
        let players = Players {
            player_1: Player::new(1, "White".to_string(), PieceColor::White, true),
            player_2: Player::new(2, "Black".to_string(), PieceColor::Black, true),
        };

        assert_eq!(players.current(PieceColor::White).id, 1);
        assert_eq!(players.current(PieceColor::Black).id, 2);
    }
}
