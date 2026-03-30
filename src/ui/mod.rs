//! UI component entry point
//!
//! Assembles all Bevy Egui plugin logic for the application.

pub mod auth;
pub mod game_ui;
// pub mod inspector;
pub mod multiplayer_menu;
pub mod promotion_ui;
#[cfg(feature = "solana")]
pub mod solana_panel;
pub mod styles;
pub mod system_params;

use auth::AuthUiPlugin;
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AuthUiPlugin);
        // Additional UI-wide registrations can go here
    }
}
