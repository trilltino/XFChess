//! Selection resource for tracking selected pieces

use bevy::prelude::*;

/// Resource to store currently selected piece
#[derive(Resource, Debug, Default)]
pub struct Selection {
    pub selected_entity: Option<Entity>,
    pub selected_position: Option<(u8, u8)>,
    pub possible_moves: Vec<(u8, u8)>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.selected_entity = None;
        self.selected_position = None;
        self.possible_moves.clear();
    }

    pub fn is_selected(&self) -> bool {
        self.selected_entity.is_some()
    }
}
