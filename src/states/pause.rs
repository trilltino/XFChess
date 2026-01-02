//! Pause menu plugin
//!
//! Displayed when pressing ESC during gameplay.
//! Allows resuming, accessing settings, or returning to main menu.

use crate::core::{GameState, PreviousState};
use crate::ui::styles::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

/// Plugin for pause menu state
pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), setup_pause_camera)
            .add_systems(
                EguiPrimaryContextPass,
                pause_ui_wrapper.run_if(in_state(GameState::Paused)),
            )
            .add_systems(
                Update,
                handle_pause_input.run_if(in_state(GameState::Paused)),
            );
    }
}

/// Wrapper for pause_ui that handles Result
fn pause_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<GameState>>,
    previous_state: ResMut<PreviousState>,
) {
    let _ = pause_ui(contexts, next_state, previous_state);
}

/// Marker component for pause camera
#[derive(Component)]
struct PauseCamera;

/// Setup camera for pause screen
/// Uses the persistent Egui camera and updates its transform
/// Handles case where camera might not exist yet
fn setup_pause_camera(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<PauseCamera>),
    >,
    mut commands: Commands,
) {
    info!(
        "[PAUSE] DEBUG: Persistent camera entity: {:?}",
        persistent_camera.entity
    );

    // Update persistent camera transform for pause view
    if let Some(camera_entity) = persistent_camera.entity {
        info!(
            "[PAUSE] DEBUG: Attempting to query camera entity {:?}",
            camera_entity
        );
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[PAUSE] DEBUG: Successfully queried camera transform");
                *transform = Transform::from_xyz(0.0, 15.0, 10.0)
                    .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
                info!("[PAUSE] Updated persistent camera transform for pause");

                // Add pause marker to persistent camera
                info!("[PAUSE] DEBUG: Adding PauseCamera component");
                commands.entity(camera_entity).insert(PauseCamera);
                info!("[PAUSE] DEBUG: PauseCamera component added successfully");
            }
            Err(e) => {
                error!(
                    "[PAUSE] ERROR: Persistent camera entity {:?} exists but query failed: {:?}",
                    camera_entity, e
                );
                error!(
                    "[PAUSE] ERROR: Query filter: With<PrimaryEguiContext>, Without<PauseCamera>"
                );
            }
        }
    } else {
        error!("[PAUSE] ERROR: Persistent camera not yet created. Entity: None");
        error!("[PAUSE] ERROR: This should not happen for Pause state (not default state)");
    }

    info!("[PAUSE] Camera setup complete");
}

/// Handle ESC key - return to main menu from pause
fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        info!("[PAUSE] ESC pressed, returning to main menu");
        next_state.set(GameState::MainMenu);
    }
}

/// Pause menu UI
fn pause_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
    mut previous_state: ResMut<PreviousState>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    // Semi-transparent overlay
    egui::CentralPanel::default()
        .frame(StyledPanel::overlay())
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                Layout::section_space(ui);

                ui.heading(TextStyle::heading("Game Paused", TextSize::LG));

                Layout::section_space(ui);

                // Resume
                if StyledButton::primary(ui, "Resume Game").clicked() {
                    info!("[PAUSE] Resuming game");
                    next_state.set(GameState::InGame);
                }

                Layout::item_space(ui);

                // Settings
                if StyledButton::secondary(ui, "Settings").clicked() {
                    previous_state.state = GameState::Paused;
                    next_state.set(GameState::Settings);
                }

                Layout::item_space(ui);

                // Main Menu
                if StyledButton::danger(ui, "Main Menu").clicked() {
                    info!("[PAUSE] Returning to main menu");
                    next_state.set(GameState::MainMenu);
                }

                Layout::section_space(ui);

                ui.label(TextStyle::caption("Press ESC to return to main menu"));
            });
        });

    Ok(())
}
