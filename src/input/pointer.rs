use crate::game::components::GamePhase;
use crate::game::resources::{CurrentGamePhase, CurrentTurn, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::picking::events::{PointerOut, PointerOver};
use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};

#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct CursorStyle {
    pub current: CursorIcon,
    pub active_hovers: std::collections::HashSet<Entity>,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self {
            current: CursorIcon::System(SystemCursorIcon::Default),
            active_hovers: Default::default(),
        }
    }
}

impl CursorStyle {
    pub fn update(&mut self) {
        self.current = if !self.active_hovers.is_empty() {
            CursorIcon::System(SystemCursorIcon::Pointer)
        } else {
            CursorIcon::System(SystemCursorIcon::Default)
        };
    }
}

pub fn on_piece_hover(
    hover: On<PointerOver>,
    piece_query: Query<&Piece>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
    mut cursor_style: Option<ResMut<CursorStyle>>,
) {
    let entity = hover.entity;
    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }
    if let Ok(piece) = piece_query.get(entity) {
        if piece.color != current_turn.color {
            return;
        }
        if let Some(ref mut cs) = cursor_style {
            cs.active_hovers.insert(entity);
            cs.update();
        }
    }
}

pub fn on_piece_unhover(unhover: On<PointerOut>, mut cursor_style: Option<ResMut<CursorStyle>>) {
    if let Some(ref mut cs) = cursor_style {
        cs.active_hovers.remove(&unhover.entity);
        cs.update();
    }
}

pub fn on_square_hover(
    hover: On<PointerOver>,
    square_query: Query<&Square>,
    selection: Res<Selection>,
    game_phase: Res<CurrentGamePhase>,
    mut cursor_style: Option<ResMut<CursorStyle>>,
) {
    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }
    if !selection.is_selected() {
        return;
    }
    let entity = hover.entity;
    if let Ok(square) = square_query.get(entity) {
        if selection.possible_moves.contains(&(square.x, square.y)) {
            if let Some(ref mut cs) = cursor_style {
                cs.active_hovers.insert(entity);
                cs.update();
            }
        }
    }
}

pub fn on_square_unhover(unhover: On<PointerOut>, mut cursor_style: Option<ResMut<CursorStyle>>) {
    if let Some(ref mut cs) = cursor_style {
        cs.active_hovers.remove(&unhover.entity);
        cs.update();
    }
}
