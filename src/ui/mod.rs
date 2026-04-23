//! UI component entry point
//!
//! Assembles all Bevy Egui plugin logic for the application.

pub mod account;
pub mod game;
pub mod menus;
pub mod spectator_mode;
pub mod styles;
pub mod system_params;

pub use account::auth;
#[cfg(feature = "solana")]
pub use account::profile_creation;
#[cfg(feature = "solana")]
pub use account::solana_panel;

pub use game::game_2d;
pub use game::game_ui;
pub use game::promotion_ui;

pub use menus::compliance_modal;
pub use menus::multiplayer_menu;
pub use menus::popup;
pub use menus::stats;
// pub use menus::inspector;

use auth::AuthUiPlugin;
use bevy::prelude::*;
use spectator_mode::SpectatorModePlugin;
#[cfg(feature = "solana")]
use crate::core::states::GameState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AuthUiPlugin);
        app.add_plugins(compliance_modal::CompliancePlugin);
        app.add_plugins(popup::PopupPlugin);
        app.add_plugins(stats::StatsPlugin);
        app.add_plugins(SpectatorModePlugin);
    }
}
