//! Full-screen lobby flows reached from the menu: Braid and P2P lobbies, the
//! tournament browser, popups, and (Solana builds) the wagered lobby.

use super::*;
#[cfg(feature = "solana")]
use crate::core::GameMode;
#[cfg(feature = "solana")]
use crate::game::ai::AiMode;
use crate::multiplayer::network::p2p::P2PConnectionStatus;
#[cfg(feature = "solana")]
use crate::multiplayer::network::vps::TournamentGameState;
#[cfg(feature = "solana")]
use crate::multiplayer::solana::tournament::GameTab;
#[cfg(feature = "solana")]
use crate::ui::styles::Layout;
use bevy_egui::egui;
use tracing::{error, info, warn};

#[cfg(feature = "solana")]
use crate::multiplayer::solana::lobby::{
    spawn_cancel_lobby, spawn_create_game, spawn_join_game, spawn_lookup_game,
    spawn_poll_game_start, spawn_poll_opponent_joined, LobbyMode, LobbyStatus,
};

#[cfg(feature = "solana")]
use crate::multiplayer::solana::tauri_signer;

mod braid_lobby;
mod p2p;
mod popups;
#[cfg(feature = "solana")]
mod solana_lobby;
#[cfg(feature = "solana")]
mod solana_tabs;
mod tournament_browser;
pub(super) use braid_lobby::*;
pub(super) use p2p::*;
pub(super) use popups::*;
#[cfg(feature = "solana")]
pub(super) use solana_lobby::*;
#[cfg(feature = "solana")]
use solana_tabs::*;
pub(super) use tournament_browser::*;
