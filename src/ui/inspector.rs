//! Inspector UI for debugging and development
//!
//! Provides a comprehensive inspector with:
//! - Left panel: Entity hierarchy browser (like Unity/Unreal)
//! - Right panel: Selected entity component inspector
//! - Bottom panel: Game resources and state
//!
//! Toggled with the F1 key.
//!
//! # Implementation Notes
//!
//! This system uses the `world: &mut World` parameter pattern which is the
//! idiomatic approach for bevy-inspector-egui integration. This allows direct
//! world access for querying resources and entities, and is compatible with the
//! EguiPrimaryContextPass schedule requirement.
//!
//! # Reference
//!
//! Pattern based on:
//! - `reference/bevy-inspector-egui/examples/integrations/side_panel.rs`
//! - `reference/bevy-inspector-egui/examples/basic/resource_inspector_manual.rs`

use bevy::prelude::*;
use bevy_egui::{EguiContext, egui, PrimaryEguiContext};
use bevy_inspector_egui::bevy_inspector::hierarchy::SelectedEntities;
use crate::game::resources::*;
use std::ops::DerefMut;

/// System that renders the comprehensive inspector UI
///
/// Provides three panels:
/// - Left: Entity hierarchy browser
/// - Right: Selected entity component details
/// - Bottom: Game resources and state
///
/// Uses World parameter for direct resource access, following the bevy-inspector-egui
/// pattern. This avoids context initialization issues and works correctly with
/// EguiPrimaryContextPass scheduling.
pub fn inspector_ui(world: &mut World, mut selected_entities: Local<SelectedEntities>) {
    // Query for the primary egui context (idiomatic bevy-inspector-egui pattern)
    let Ok(mut ctx) = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single_mut(world)
    else {
        return;
    };

    // Clone the context as required by bevy-inspector-egui
    let mut egui_context = ctx.deref_mut().clone();

    // Left panel: Entity hierarchy browser
    egui::SidePanel::left("hierarchy")
        .default_width(250.0)
        .show(egui_context.get_mut(), |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.heading("Entity Hierarchy");
                ui.label("Press F1 to toggle");
                ui.separator();

                bevy_inspector_egui::bevy_inspector::hierarchy::hierarchy_ui(
                    world,
                    ui,
                    &mut selected_entities,
                );

                ui.allocate_space(ui.available_size());
            });
        });

    // Right panel: Selected entity component inspector
    egui::SidePanel::right("entity_inspector")
        .default_width(300.0)
        .show(egui_context.get_mut(), |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.heading("Component Inspector");
                ui.separator();

                match selected_entities.as_slice() {
                    &[entity] => {
                        bevy_inspector_egui::bevy_inspector::ui_for_entity(world, entity, ui);
                    }
                    entities if !entities.is_empty() => {
                        bevy_inspector_egui::bevy_inspector::ui_for_entities_shared_components(
                            world, entities, ui,
                        );
                    }
                    _ => {
                        ui.label("No entity selected");
                        ui.label("");
                        ui.label("Select an entity from the hierarchy");
                        ui.label("to inspect its components");
                    }
                }

                ui.allocate_space(ui.available_size());
            });
        });

    // Bottom panel: Game resources and state
    egui::TopBottomPanel::bottom("game_resources")
        .default_height(250.0)
        .show(egui_context.get_mut(), |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.heading("Game State Resources");
                ui.separator();

                // Query resources from world (must be done before showing UI)
                let current_turn = world.resource::<CurrentTurn>();
                let game_timer = world.resource::<GameTimer>();
                let move_history = world.resource::<MoveHistory>();
                let game_phase = world.resource::<CurrentGamePhase>();
                let selection = world.resource::<Selection>();

                // Extract data from resources (to avoid borrowing issues)
                let turn_color = current_turn.color;
                let turn_move_number = current_turn.move_number;
                let timer_white = game_timer.white_time_left;
                let timer_black = game_timer.black_time_left;
                let timer_increment = game_timer.increment;
                let timer_running = game_timer.is_running;
                let phase_value = game_phase.0;
                let selected_pos = selection.selected_position;
                // Only need the count, not the full Vec - no need to clone
                let possible_moves_count = selection.possible_moves.len();
                let history_len = move_history.len();
                let last_move = move_history.last_move().copied();

                ui.columns(3, |columns| {
                    // Column 1: Turn and Phase
                    columns[0].heading("Current Turn");
                    columns[0].label(format!("Color: {:?}", turn_color));
                    columns[0].label(format!("Move #: {}", turn_move_number));
                    columns[0].add_space(10.0);
                    columns[0].heading("Game Phase");
                    columns[0].label(format!("{:?}", phase_value));

                    // Column 2: Timer and Selection
                    columns[1].heading("Game Timer");
                    columns[1].label(format!("White: {:.1}s", timer_white));
                    columns[1].label(format!("Black: {:.1}s", timer_black));
                    columns[1].label(format!("Increment: {:.1}s", timer_increment));
                    columns[1].label(format!("Running: {}", timer_running));
                    columns[1].add_space(10.0);
                    columns[1].heading("Selection");
                    if let Some(pos) = selected_pos {
                        columns[1].label(format!("Selected: ({}, {})", pos.0, pos.1));
                        columns[1].label(format!("Possible moves: {}", possible_moves_count));
                    } else {
                        columns[1].label("No piece selected");
                    }

                    // Column 3: Move History
                    columns[2].heading("Move History");
                    columns[2].label(format!("Total moves: {}", history_len));
                    if let Some(last) = last_move {
                        columns[2].label("Last move:");
                        columns[2].label(format!("  {:?} {:?}", last.piece_color, last.piece_type));
                        columns[2].label(format!("  From: ({}, {})", last.from.0, last.from.1));
                        columns[2].label(format!("  To: ({}, {})", last.to.0, last.to.1));
                        if let Some(captured) = last.captured {
                            columns[2].label(format!("  Captured: {:?}", captured));
                        }
                    }
                });

                ui.allocate_space(ui.available_size());
            });
        });
}
