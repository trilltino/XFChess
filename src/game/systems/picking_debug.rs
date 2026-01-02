//! Picking debug system
//!
//! Provides comprehensive debugging for pointer interactions and picking events.

use crate::core::GameState;
use crate::game::resources::Selection;
use crate::rendering::pieces::Piece;
use crate::rendering::utils::Square;
use bevy::ecs::message::MessageReader;
use bevy::input::ButtonInput;
use bevy::picking::events::Pointer;
use bevy::picking::pointer::{PointerId, PointerInteraction};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

#[derive(Resource, Default)]
struct SelectionDebugState {
    enabled: bool,
}

fn toggle_selection_debug(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<SelectionDebugState>) {
    if keys.just_pressed(KeyCode::F9) {
        state.enabled = !state.enabled;
        if state.enabled {
            info!("[SELECTION_DEBUG] Enabled");
        } else {
            info!("[SELECTION_DEBUG] Disabled");
        }
    }
}

fn selection_debug_ui(
    mut contexts: EguiContexts,
    state: Res<SelectionDebugState>,
    selection: Res<Selection>,
    pieces: Query<&Piece>,
) {
    if !state.enabled {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Selection Debug").show(ctx, |ui| {
        if let Some(entity) = selection.selected_entity {
            ui.label(format!("Entity: {:?}", entity));
            if let Some(position) = selection.selected_position {
                ui.label(format!(
                    "Position: {}{}",
                    (b'a' + position.1) as char,
                    position.0 + 1
                ));
            }
            if let Ok(piece) = pieces.get(entity) {
                ui.label(format!("Piece: {:?} {:?}", piece.color, piece.piece_type));
            }
            ui.label(format!("Dragging: {}", selection.is_dragging));
            if let Some(start) = selection.drag_start {
                ui.label(format!(
                    "Drag start: {}{}",
                    (b'a' + start.1) as char,
                    start.0 + 1
                ));
            }
            ui.separator();
            if selection.possible_moves.is_empty() {
                ui.label("Possible moves: none");
            } else {
                ui.label(format!(
                    "Possible moves: {}",
                    selection.possible_moves.len()
                ));
                egui::ScrollArea::vertical()
                    .id_salt("selection_moves")
                    .show(ui, |ui| {
                        for (x, y) in &selection.possible_moves {
                            ui.label(format!("{}{}", (b'a' + *y) as char, x + 1));
                        }
                    });
            }
        } else {
            ui.label("No selection");
        }
    });
}

/// Debug all clicks - rate-limited to avoid console spam
pub fn debug_all_clicks(
    mut click_reader: MessageReader<Pointer<Click>>,
    pieces: Query<(Entity, &Piece)>,
    squares: Query<(Entity, &Square)>,
    selection: Res<Selection>,
    time: Res<Time>,
    mut last_log: Local<f32>,
) {
    // Rate limit to once per 5 seconds
    *last_log += time.delta_secs();

    for event in click_reader.read() {
        if *last_log < 5.0 {
            continue; // Skip logging if too frequent
        }
        *last_log = 0.0;

        // Check if it's a piece
        if let Ok((_entity, piece)) = pieces.get(event.entity) {
            debug!(
                "[CLICK] {:?} {:?} at ({},{}) | Selected: {:?}",
                piece.color,
                piece.piece_type,
                piece.x,
                piece.y,
                selection.selected_entity.is_some()
            );
        } else if let Ok((_entity, square)) = squares.get(event.entity) {
            debug!(
                "[CLICK] Square ({},{}) | Selected: {:?}",
                square.x,
                square.y,
                selection.selected_entity.is_some()
            );
        }
    }
}

/// Debug system that monitors PointerInteraction component states (rate-limited)
pub fn debug_pointer_interactions(
    pieces: Query<(Entity, &Piece, &PointerInteraction), (With<Piece>, With<PointerInteraction>)>,
    squares: Query<
        (Entity, &Square, &PointerInteraction),
        (With<Square>, With<PointerInteraction>),
    >,
    time: Res<Time>,
    mut last_check: Local<f32>,
) {
    // Only check periodically (every 5 seconds) to avoid spam
    *last_check += time.delta_secs();
    if *last_check < 5.0 {
        return;
    }
    *last_check = 0.0;

    let mut active_pieces = 0;
    let mut active_squares = 0;

    // Count active interactions instead of logging each one
    for (entity, _piece, interaction) in pieces.iter() {
        if let Some((hit_entity, _hit_data)) = interaction.get_nearest_hit() {
            if *hit_entity == entity {
                active_pieces += 1;
            }
        }
    }

    for (entity, _square, interaction) in squares.iter() {
        if let Some((hit_entity, _hit_data)) = interaction.get_nearest_hit() {
            if *hit_entity == entity {
                active_squares += 1;
            }
        }
    }

    if active_pieces > 0 || active_squares > 0 {
        debug!(
            "[HOVER] {} pieces, {} squares",
            active_pieces, active_squares
        );
    }
}

/// Debug system that checks if entities have required components for picking (rate-limited)
pub fn debug_picking_setup(
    pieces: Query<(Entity, &Piece, Option<&PointerInteraction>), (With<Piece>, Without<Square>)>,
    squares: Query<(Entity, &Square, Option<&PointerInteraction>), (With<Square>, Without<Piece>)>,
    cameras: Query<(Entity, &Camera3d)>,
    pointers: Query<Entity, With<PointerId>>, // Check for pointers
    time: Res<Time>,
    mut last_check: Local<f32>,
) {
    // Only check periodically (every 10 seconds) to avoid spam
    *last_check += time.delta_secs();
    if *last_check < 2.0 {
        return;
    }
    *last_check = 0.0;

    let camera_count = cameras.iter().count();
    let pointer_count = pointers.iter().count();
    let pieces_with = pieces.iter().filter(|(_, _, i)| i.is_some()).count();
    let pieces_without = pieces.iter().filter(|(_, _, i)| i.is_none()).count();
    let squares_with = squares.iter().filter(|(_, _, i)| i.is_some()).count();
    let squares_without = squares.iter().filter(|(_, _, i)| i.is_none()).count();

    info!(
        "[SETUP] Cameras: {} | Pointers: {} | Pieces: {}/{} | Squares: {}/{}",
        camera_count,
        pointer_count,
        pieces_with,
        pieces_with + pieces_without,
        squares_with,
        squares_with + squares_without
    );

    if pointer_count == 0 {
        error!("[SETUP] NO POINTERS FOUND! Picking will not work.");
    }

    if pieces_without > 0 || squares_without > 0 {
        // error!(
        //     "[SETUP] Missing PointerInteraction on {} pieces, {} squares!",
        //     pieces_without, squares_without
        // );
    }
}

/// Plugin for picking debug systems
pub struct PickingDebugPlugin;

impl Plugin for PickingDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectionDebugState>();
        app.add_systems(
            Update,
            (
                debug_all_clicks,
                debug_pointer_interactions,
                debug_picking_setup,
                toggle_selection_debug,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            EguiPrimaryContextPass,
            selection_debug_ui.run_if(in_state(GameState::InGame)),
        );
    }
}
