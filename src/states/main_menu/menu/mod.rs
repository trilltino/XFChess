//! The main menu's panels and scene: background board ([`background`]), window
//! frame ([`frame`]), navigation panels ([`panels`]), updates ([`update_panel`]),
//! account and wallet ([`account`]) and shared widgets ([`widgets`]).

use bevy::prelude::*;
use bevy_egui::egui;

use crate::core::update_check::{self, Platform, UpdateStatus};
use crate::core::{GameMode, GameState, MenuState};
use crate::game::resources::MenuSounds;
use crate::rendering::pieces::{PieceColor, PieceMeshes, PieceType};
use crate::states::main_menu::{CompetitiveMenuState, MainMenuModal};
use crate::ui::system_params::MainMenuUIContext;

mod account;
mod background;
mod frame;
mod panels;
mod update_panel;
mod widgets;
pub use account::*;
pub use background::*;
pub use frame::*;
use panels::*;
use update_panel::*;
pub(super) use widgets::*;

#[derive(Resource, Default)]
pub struct MenuExitConfirm {
    pub visible: bool,
}

#[derive(Resource, Default)]
pub struct MenuFocusMode {
    pub active: bool,
}

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum MenuPanel {
    #[default]
    Main,
    PlayOnline,
    Puzzles,
    Tournaments,
    SolanaConnect,
    DirectConnection,
    Settings,
    Profile,
    Multiplayer,
    Updates,
}
