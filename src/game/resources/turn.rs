//! Turn tracking resource

use bevy::prelude::*;
use crate::rendering::pieces::PieceColor;

/// Tracks whose turn it currently is
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CurrentTurn {
    pub color: PieceColor,
    pub move_number: u32,
}

impl Default for CurrentTurn {
    fn default() -> Self {
        Self {
            color: PieceColor::White,
            move_number: 1,
        }
    }
}

impl CurrentTurn {
    pub fn switch(&mut self) {
        self.color = match self.color {
            PieceColor::White => {
                self.move_number += 1;
                PieceColor::Black
            }
            PieceColor::Black => PieceColor::White,
        };
    }
}

/// Resource to track the current game phase
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CurrentGamePhase(pub crate::game::components::GamePhase);

impl Default for CurrentGamePhase {
    fn default() -> Self {
        Self(crate::game::components::GamePhase::Playing)
    }
}
