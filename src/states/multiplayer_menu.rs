//! Multiplayer menu plugin with Braid P2P integration
//!
//! Displays the multiplayer lobby with options to:
//! - Find opponents via gossip matchmaking
//! - Connect directly via Braid URI
//! - Play against AI
//!
//! This plugin wires up the UI system from [`crate::ui::multiplayer_menu`].

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

use crate::core::GameState;
use crate::ui::multiplayer_menu::{multiplayer_menu_system, MultiplayerMenuState};

/// Plugin for multiplayer menu state
///
/// Registers the multiplayer menu UI system to run during `EguiPrimaryContextPass`
/// when the game is in `GameState::MultiplayerMenu`.
pub struct MultiplayerMenuPlugin;

impl Plugin for MultiplayerMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MultiplayerMenuState>().add_systems(
            EguiPrimaryContextPass,
            multiplayer_menu_system.run_if(in_state(GameState::MultiplayerMenu)),
        );
    }
}
