use crate::rendering::pieces::PieceColor;
use bevy::prelude::*;

pub mod promotion;
pub mod selection;
pub use promotion::*;
pub use selection::*;

#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Player {
    pub id: u8,

    pub name: String,

    pub color: PieceColor,

    pub is_human: bool,
}

impl Player {
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

#[derive(Resource, Debug, Reflect, Default)]
#[reflect(Resource)]
pub struct Players {
    pub player_1: Player,

    pub player_2: Player,
}

impl Players {
    pub fn current(&self, current_color: PieceColor) -> &Player {
        match current_color {
            PieceColor::White => &self.player_1,
            PieceColor::Black => &self.player_2,
        }
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
}
