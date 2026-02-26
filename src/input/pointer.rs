//! Advanced pointer interaction system with hover effects and cursor management

use crate::game::components::GamePhase;
use crate::game::resources::{CurrentGamePhase, CurrentTurn, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

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
pub struct HoverMaterials {
    /// Material for pieces when hovered (brightened)
    pub piece_hover_factor: f32,
    /// Material for squares when hovered (highlighted)
    pub square_hover_factor: f32,
}

impl Default for HoverMaterials {
    fn default() -> Self {
        Self {
            piece_hover_factor: 1.3,  // 30% brighter
            square_hover_factor: 1.2, // 20% brighter
        }
    }
}

/// Resource for storing original material states before hover modifications
#[derive(Resource, Default, Debug)]
pub struct OriginalMaterials {
    /// Map of entity -> original material handle
    pub materials: std::collections::HashMap<Entity, Handle<StandardMaterial>>,
}

/// System that tracks cursor position in real-time
pub fn cursor_tracking_system(
    q_windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut cursor_state: ResMut<CursorState>,
) {
    cursor_state.last_update += time.delta_secs();
    if let Ok(window) = q_windows.single() {
        cursor_state.position = window.cursor_position();
        if cursor_state.last_update >= 1.0 {
            if let Some(position) = cursor_state.position {
                trace!(
                    "[POINTER] Cursor position: ({:.1}, {:.1})",
                    position.x,
                    position.y
                );
            }
            cursor_state.last_update = 0.0;
        }
    } else {
        cursor_state.position = None;
    }
}

/// Observer function for piece hover events (Pointer<Over>)
pub fn on_piece_hover(
    hover: On<Pointer<Over>>,
    piece_query: Query<&Piece>,
    current_turn: Res<CurrentTurn>,
    game_phase: Res<CurrentGamePhase>,
    hover_materials: Res<HoverMaterials>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
    mut cursor_style: ResMut<CursorStyle>,
) {
    let entity = hover.entity;
    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }
    if let Ok(piece) = piece_query.get(entity) {
        if piece.color != current_turn.color {
            return;
        }
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            original_materials
                .materials
                .entry(entity)
                .or_insert_with(|| material_handle.0.clone());

            if let Some(original_mat) = materials.get(&material_handle.0) {
                let mut brightened = original_mat.clone();

                let rgb = brightened.base_color.to_linear();
                brightened.base_color = Color::LinearRgba(LinearRgba {
                    red: (rgb.red * hover_materials.piece_hover_factor).min(1.0),
                    green: (rgb.green * hover_materials.piece_hover_factor).min(1.0),
                    blue: (rgb.blue * hover_materials.piece_hover_factor).min(1.0),
                    alpha: rgb.alpha,
                });

                let brightened_handle = materials.add(brightened);
                material_handle.0 = brightened_handle;

                trace!(
                    "[POINTER] Hover effect applied to {:?} piece at entity {:?}",
                    piece.color,
                    entity
                );

                cursor_style.active_hovers.insert(entity);
                cursor_style.update();
            }
        }
    }
}

/// Observer function for piece unhover events (Pointer<Out>)
pub fn on_piece_unhover(
    unhover: On<Pointer<Out>>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
    mut cursor_style: ResMut<CursorStyle>,
) {
    let entity = unhover.entity;

    if let Some(original_handle) = original_materials.materials.remove(&entity) {
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            material_handle.0 = original_handle;
            trace!("[POINTER] Hover effect removed from entity {:?}", entity);
        }
    }

    cursor_style.active_hovers.remove(&entity);
    cursor_style.update();
}

/// Observer function for square hover events (Pointer<Over>)
pub fn on_square_hover(
    hover: On<Pointer<Over>>,
    square_query: Query<&Square>,
    selection: Res<Selection>,
    game_phase: Res<CurrentGamePhase>,
    hover_materials: Res<HoverMaterials>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
    mut cursor_style: ResMut<CursorStyle>,
) {
    let entity = hover.entity;

    if !matches!(game_phase.0, GamePhase::Playing | GamePhase::Check) {
        return;
    }

    if !selection.is_selected() {
        return;
    }

    if let Ok(square) = square_query.get(entity) {
        let square_pos = (square.x, square.y);

        if !selection.possible_moves.contains(&square_pos) {
            return;
        }

        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            original_materials
                .materials
                .entry(entity)
                .or_insert_with(|| material_handle.0.clone());

            if let Some(original_mat) = materials.get(&material_handle.0) {
                let mut highlighted = original_mat.clone();

                let rgb = highlighted.base_color.to_linear();
                highlighted.base_color = Color::LinearRgba(LinearRgba {
                    red: (rgb.red * hover_materials.square_hover_factor).min(1.0),
                    green: (rgb.green * hover_materials.square_hover_factor).min(1.0),
                    blue: (rgb.blue * hover_materials.square_hover_factor).min(1.0),
                    alpha: rgb.alpha,
                });

                let highlighted_handle = materials.add(highlighted);
                material_handle.0 = highlighted_handle;

                trace!(
                    "[POINTER] Hover effect applied to square ({}, {}) at entity {:?}",
                    square.x,
                    square.y,
                    entity
                );
            }

            cursor_style.active_hovers.insert(entity);
            cursor_style.update();
        }
    }
}

/// Observer function for square unhover events (Pointer<Out>)
pub fn on_square_unhover(
    unhover: On<Pointer<Out>>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut original_materials: ResMut<OriginalMaterials>,
    mut cursor_style: ResMut<CursorStyle>,
) {
    let entity = unhover.entity;

    if let Some(original_handle) = original_materials.materials.remove(&entity) {
        if let Ok(mut material_handle) = material_query.get_mut(entity) {
            material_handle.0 = original_handle;
            trace!(
                "[POINTER] Hover effect removed from square at entity {:?}",
                entity
            );
        }
    }

    cursor_style.active_hovers.remove(&entity);
    cursor_style.update();
}

/// System that manages cursor icon changes based on hover state
pub fn cursor_style_system(
    cursor_style: Res<CursorStyle>,
    mut commands: Commands,
    mut q_window: Query<(Entity, Option<&mut CursorIcon>), With<PrimaryWindow>>,
) {
    if let Some((entity, maybe_icon)) = q_window.iter_mut().next() {
        if let Some(mut icon) = maybe_icon {
            if *icon != cursor_style.current {
                *icon = cursor_style.current.clone();
            }
        } else {
            commands.entity(entity).insert(cursor_style.current.clone());
        }
    }
}

/// Debug system that logs pointer interactions (rate-limited)
#[derive(Resource, Debug)]
pub struct PointerDebugTimer {
    pub time: f32,
}

impl Default for PointerDebugTimer {
    fn default() -> Self {
        Self { time: 0.0 }
    }
}
