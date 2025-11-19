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

use crate::game::resources::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, PrimaryEguiContext};
use bevy_inspector_egui::bevy_inspector::hierarchy::SelectedEntities;
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
                // Handle missing resources gracefully (they may not exist in menu states)
                let (turn_color, turn_move_number) = world
                    .get_resource::<CurrentTurn>()
                    .map(|t| (t.color, t.move_number))
                    .unwrap_or((crate::rendering::pieces::PieceColor::White, 0));

                let (timer_white, timer_black, timer_increment, timer_running) = world
                    .get_resource::<GameTimer>()
                    .map(|t| {
                        (
                            t.white_time_left,
                            t.black_time_left,
                            t.increment,
                            t.is_running,
                        )
                    })
                    .unwrap_or((0.0, 0.0, 0.0, false));

                let (history_len, last_move) = world
                    .get_resource::<MoveHistory>()
                    .map(|h| (h.len(), h.last_move().copied()))
                    .unwrap_or((0, None));

                let phase_value = world
                    .get_resource::<CurrentGamePhase>()
                    .map(|p| p.0)
                    .unwrap_or(crate::game::components::GamePhase::Setup);

                let (selected_pos, possible_moves_count) = world
                    .get_resource::<Selection>()
                    .map(|s| (s.selected_position, s.possible_moves.len()))
                    .unwrap_or((None, 0));

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
