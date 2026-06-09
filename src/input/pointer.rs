//! Advanced pointer interaction system with hover effects and cursor management

use crate::game::components::GamePhase;
use crate::game::resources::{CurrentGamePhase, CurrentTurn, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};

/// Resource tracking the current cursor position within the game window
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct CursorState {
    /// Current cursor position within window bounds, None if outside
    pub position: Option<Vec2>,
    /// Time accumulator for debug logging rate-limiting
    pub last_update: f32,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            position: None,
            last_update: 0.0,
        }
    }
}

/// Resource tracking the current cursor icon style
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct CursorStyle {
    /// Current cursor icon being displayed
    pub current: CursorIcon,
    /// Set of entities currently triggering a pointer cursor
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

/// Resource for caching material handles to avoid repeated asset lookups
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct HoverMaterials;

impl Default for HoverMaterials {
    fn default() -> Self { Self }
}

/// Observer function for piece hover events (Pointer<Over>)
pub fn on_piece_hover(
    hover: On<Pointer<Over>>,
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

/// Observer function for piece unhover events (Pointer<Out>)
pub fn on_piece_unhover(
    unhover: On<Pointer<Out>>,
    mut cursor_style: Option<ResMut<CursorStyle>>,
) {
    if let Some(ref mut cs) = cursor_style {
        cs.active_hovers.remove(&unhover.entity);
        cs.update();
    }
}

/// Observer function for square hover events (Pointer<Over>)
pub fn on_square_hover(
    hover: On<Pointer<Over>>,
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

/// Observer function for square unhover events (Pointer<Out>)
pub fn on_square_unhover(
    unhover: On<Pointer<Out>>,
    mut cursor_style: Option<ResMut<CursorStyle>>,
) {
    if let Some(ref mut cs) = cursor_style {
        cs.active_hovers.remove(&unhover.entity);
        cs.update();
    }
}
