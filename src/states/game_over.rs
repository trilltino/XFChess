//! Game over screen plugin
//!
//! Displayed when a game concludes (checkmate, stalemate, or resignation).
//! Shows game results, statistics, and options for next actions.

use crate::core::{GameState, GameStatistics};
use crate::game::resources::GameOverState;
use crate::ui::styles::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

/// Plugin for game over screen
pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::GameOver),
            (setup_game_over_camera, record_game_stats),
        )
        .add_systems(
            EguiPrimaryContextPass,
            game_over_ui_wrapper.run_if(in_state(GameState::GameOver)),
        );
    }
}

/// Wrapper for game_over_ui that handles Result
fn game_over_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<GameState>>,
    game_over: Res<GameOverState>,
    stats: Res<GameStatistics>,
) {
    let _ = game_over_ui(contexts, next_state, game_over, stats);
}

/// Marker component for game over camera
#[derive(Component)]
struct GameOverCamera;

/// Setup camera for game over screen
/// Uses the persistent Egui camera and updates its transform
/// Handles case where camera might not exist yet
fn setup_game_over_camera(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<GameOverCamera>),
    >,
    mut commands: Commands,
) {
    info!(
        "[GAME_OVER] DEBUG: Persistent camera entity: {:?}",
        persistent_camera.entity
    );

    // Update persistent camera transform for game over view
    if let Some(camera_entity) = persistent_camera.entity {
        info!(
            "[GAME_OVER] DEBUG: Attempting to query camera entity {:?}",
            camera_entity
        );
        match camera_query.get_mut(camera_entity) {
            Ok(mut transform) => {
                info!("[GAME_OVER] DEBUG: Successfully queried camera transform");
                *transform = Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y);
                info!("[GAME_OVER] Updated persistent camera transform for game over");

                // Add game over marker to persistent camera
                info!("[GAME_OVER] DEBUG: Adding GameOverCamera component");
                commands.entity(camera_entity).insert(GameOverCamera);
                info!("[GAME_OVER] DEBUG: GameOverCamera component added successfully");
            }
            Err(e) => {
                error!("[GAME_OVER] ERROR: Persistent camera entity {:?} exists but query failed: {:?}", camera_entity, e);
                error!("[GAME_OVER] ERROR: Query filter: With<PrimaryEguiContext>, Without<GameOverCamera>");
            }
        }
    } else {
        error!("[GAME_OVER] ERROR: Persistent camera not yet created. Entity: None");
        error!("[GAME_OVER] ERROR: This should not happen for GameOver state (not default state)");
    }

    info!("[GAME_OVER] Camera setup complete");
}

/// Record game statistics when entering game over state
fn record_game_stats(_game_over: Res<GameOverState>, mut stats: ResMut<GameStatistics>) {
    // Extract winner and move count from game over state
    // Note: This is simplified - you'd extract actual data from your game
    let winner = None; // TODO: Extract from game_over.winner
    let moves = 0; // TODO: Extract from move history

    stats.record_game(winner, moves);
    info!("[GAME_OVER] Game statistics recorded");
}

/// Game over UI
fn game_over_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
    game_over: Res<GameOverState>,
    stats: Res<GameStatistics>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    egui::CentralPanel::default()
        .frame(egui::Frame {
            fill: UiColors::BG_DARK,
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                Layout::section_space(ui);

                // Game result
                ui.heading(TextStyle::heading("Game Over", TextSize::XL));

                Layout::item_space(ui);

                // Result message
                let result_text = game_over.message();
                ui.label(TextStyle::heading(result_text, TextSize::LG));

                Layout::section_space(ui);

                // Game statistics
                StyledPanel::card().show(ui, |ui| {
                    ui.heading(TextStyle::heading("Your Statistics", TextSize::MD));
                    Layout::item_space(ui);

                    ui.label(TextStyle::body(format!(
                        "Games Played: {}",
                        stats.games_played
                    )));
                    ui.label(TextStyle::body(format!(
                        "White Wins: {} ({:.1}%)",
                        stats.white_wins,
                        stats.win_rate_white()
                    )));
                    ui.label(TextStyle::body(format!(
                        "Black Wins: {} ({:.1}%)",
                        stats.black_wins,
                        stats.win_rate_black()
                    )));
                    ui.label(TextStyle::body(format!("Draws: {}", stats.draws)));
                    ui.label(TextStyle::body(format!(
                        "Average Moves: {:.1}",
                        stats.average_moves()
                    )));
                });

                Layout::section_space(ui);

                // Action buttons
                if StyledButton::primary(ui, "New Game").clicked() {
                    info!("[GAME_OVER] Starting new game");
                    next_state.set(GameState::InGame);
                }

                Layout::item_space(ui);

                if StyledButton::secondary(ui, "Main Menu").clicked() {
                    info!("[GAME_OVER] Returning to main menu");
                    next_state.set(GameState::MainMenu);
                }
            });
        });

    Ok(())
}
