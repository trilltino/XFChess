//! Game over screen plugin
//!
//! Displayed when a game concludes (checkmate, stalemate, or resignation).
//! Shows a small translucent popup with game results and cinematic camera in background.
//! For Solana wager games, displays detailed fee breakdown from smart contract.

use crate::core::GameState;
use crate::game::camera_modes::{CameraViewMode, CinematicSequence};
use crate::game::resources::{GameOverState, MoveHistory};
use crate::game::view_mode::{PlayerViewPreferences, ViewMode};
use crate::ui::menus::game_over_popup::GameOverPopupPlugin;
use bevy::prelude::*;

/// Plugin for game over screen with cinematic camera and popup UI
pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        // Add the popup plugin
        app.add_plugins(GameOverPopupPlugin);

        // Setup camera and record stats when entering GameOver
        app.add_systems(
            OnEnter(GameState::GameOver),
            (setup_cinematic_camera, record_game_stats),
        );

        // Reset camera when leaving GameOver
        app.add_systems(
            OnExit(GameState::GameOver),
            reset_camera_on_exit,
        );
    }
}

/// Marker component for tracking that cinematic mode was auto-started
#[derive(Component)]
pub struct AutoCinematicMarker;

/// Setup cinematic camera for game over screen
/// Automatically switches to cinematic mode for dramatic effect
fn setup_cinematic_camera(
    mut camera_view_mode: ResMut<CameraViewMode>,
    mut cinematic_sequence: ResMut<CinematicSequence>,
    mut view_preferences: ResMut<PlayerViewPreferences>,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut commands: Commands,
) {
    info!("[GAME_OVER] Auto-switching to cinematic camera mode");

    // Switch to cinematic camera mode
    *camera_view_mode = CameraViewMode::Cinematic;
    view_preferences.local_view = ViewMode::Standard3D;

    // Reset and start the cinematic sequence
    cinematic_sequence.reset();

    // Mark the camera as auto-started so we can reset it later
    if let Some(camera_entity) = persistent_camera.entity {
        commands.entity(camera_entity).insert(AutoCinematicMarker);
    }

    info!("[GAME_OVER] Cinematic camera activated");
}

/// Reset camera to default mode when leaving game over state
fn reset_camera_on_exit(
    mut camera_view_mode: ResMut<CameraViewMode>,
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    marked_cameras: Query<Entity, With<AutoCinematicMarker>>,
) {
    info!("[GAME_OVER] Resetting camera to default mode");

    // Reset camera view mode to Default
    *camera_view_mode = CameraViewMode::Default;

    // Remove the auto-cinematic marker from camera
    for entity in marked_cameras.iter() {
        commands.entity(entity).remove::<AutoCinematicMarker>();
    }

    // Also try to remove from persistent camera if it exists
    if let Some(camera_entity) = persistent_camera.entity {
        commands.entity(camera_entity).remove::<AutoCinematicMarker>();
    }

    info!("[GAME_OVER] Camera reset complete");
}

/// Record game statistics when entering game over state
fn record_game_stats(
    game_over: Res<GameOverState>,
    move_history: Res<MoveHistory>,
    mut stats: ResMut<crate::core::GameStatistics>,
) {
    let winner = game_over.winner();
    let moves = move_history.len() as u32;

    stats.record_game(winner, moves);
    info!(
        "[GAME_OVER] Game statistics recorded: winner={:?}, moves={}",
        winner, moves
    );
}

