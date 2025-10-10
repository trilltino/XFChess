//! Move history resource

use bevy::prelude::*;
use crate::game::components::MoveRecord;

/// Resource to store move history
#[derive(Resource, Debug, Default)]
pub struct MoveHistory {
    pub moves: Vec<MoveRecord>,
}

impl MoveHistory {
    pub fn add_move(&mut self, record: MoveRecord) {
        self.moves.push(record);
    }

    /// Get the last move made
    /// TODO: Will be used for move validation and UI display
    #[allow(dead_code)]
    pub fn last_move(&self) -> Option<&MoveRecord> {
        self.moves.last()
    }

    /// Check if there are moves that can be undone
    /// TODO: Will be used for undo feature
    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.moves.is_empty()
    }
}
