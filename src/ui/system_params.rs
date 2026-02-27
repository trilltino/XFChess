//! System parameter groups for UI systems
//!
//! Provides convenient SystemParam types that group related resources together
//! for UI rendering, following the bevy_egui pattern of using SystemParams
//! for cleaner APIs.

use crate::assets::{GameAssets, LoadingProgress};
use crate::core::{GameMode as CoreGameMode, GameSettings, GameState, PreviousState};
use crate::game::ai::ChessAIResource;
use crate::game::resources::{AIParams, GameStateParams};
use crate::game::resources::{GameTimer, MoveHistory, Players};
use crate::game::view_mode::ViewMode;
use crate::multiplayer::p2p_connection::{
    ConnectToPeerEvent, HostGameEvent, P2PConnectionState, P2PUIState,
};
#[cfg(feature = "solana")]
use crate::multiplayer::solana_addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
use crate::multiplayer::{BraidNetworkState, BraidP2PConfig};
use crate::states::main_menu::{CompetitiveMenuState, MenuExpanded};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

/// System parameter grouping main menu UI resources
///
/// Bundles all resources needed by the main menu UI to avoid exceeding
/// Bevy's system parameter limit (16 parameters).
///
/// # Resources Included
///
/// - [`EguiContexts`] - UI rendering contexts
/// - [`NextState<GameState>`] - For transitioning game states
/// - [`NextState<MenuState>`] - For transitioning menu substates
/// - [`ChessAIResource`] - AI configuration
/// - [`ViewMode`] - Camera/view configuration
/// - [`LoadingProgress`] - Asset loading status
/// - [`GameAssets`] - Loaded game assets
/// - [`MenuExpanded`] - Menu expansion state
/// - [`GameSettings`] - Game settings
/// - [`CoreGameMode`] - Core game mode selection
/// - [`CompetitiveMenuState`] - Competitive match UI state
/// - [`BraidP2PConfig`] - P2P network configuration
/// - [`BraidNetworkState`] - Network connection state
/// - [`P2PUIState`] - P2P UI state
/// - [`P2PConnectionState`] - P2P connection status
/// - [`EventWriter<HostGameEvent>`] - For hosting games
/// - [`EventWriter<ConnectToPeerEvent>`] - For connecting to peers
#[derive(SystemParam)]
pub struct MainMenuUIContext<'w, 's> {
    pub contexts: EguiContexts<'w, 's>,
    pub next_state: ResMut<'w, NextState<GameState>>,
    pub menu_state: ResMut<'w, NextState<crate::core::MenuState>>,
    pub current_menu_state: Option<Res<'w, State<crate::core::MenuState>>>,
    pub ai_config: ResMut<'w, ChessAIResource>,
    pub view_mode: ResMut<'w, ViewMode>,
    pub loading_progress: ResMut<'w, LoadingProgress>,
    pub game_assets: ResMut<'w, GameAssets>,
    pub previous_state: ResMut<'w, PreviousState>,
    pub menu_expanded: ResMut<'w, MenuExpanded>,
    pub settings: ResMut<'w, GameSettings>,
    pub core_mode: ResMut<'w, CoreGameMode>,
    pub competitive_menu: ResMut<'w, CompetitiveMenuState>,
    pub braid_config: ResMut<'w, BraidP2PConfig>,
    pub network_state: Res<'w, BraidNetworkState>,
    pub p2p_ui: ResMut<'w, P2PUIState>,
    pub p2p_state: Res<'w, P2PConnectionState>,
    pub host_game_events: MessageWriter<'w, HostGameEvent>,
    pub connect_events: MessageWriter<'w, ConnectToPeerEvent>,
    #[cfg(feature = "solana")]
    pub wallet: Option<ResMut<'w, SolanaWallet>>,
    #[cfg(feature = "solana")]
    pub solana_sync: Option<ResMut<'w, SolanaGameSync>>,
    #[cfg(feature = "solana")]
    pub competitive: Option<ResMut<'w, CompetitiveMatchState>>,
}

/// System parameter grouping UI-related resources
#[derive(SystemParam)]
pub struct GameUIParams<'w, 's> {
    pub contexts: EguiContexts<'w, 's>,
    pub game_state: GameStateParams<'w>,
    pub ai_params: AIParams<'w>,
    pub move_history: Res<'w, MoveHistory>,
    pub game_timer: Res<'w, GameTimer>,
    pub players: Res<'w, Players>,
    pub next_state: ResMut<'w, NextState<GameState>>,
    pub previous_state: ResMut<'w, PreviousState>,
    pub game_mode: Res<'w, CoreGameMode>,
    #[cfg(feature = "solana")]
    pub solana_wallet: ResMut<'w, SolanaWallet>,
    #[cfg(feature = "solana")]
    pub solana_sync: ResMut<'w, SolanaGameSync>,
    #[cfg(feature = "solana")]
    pub solana_profile: Res<'w, SolanaProfile>,
    #[cfg(feature = "solana")]
    pub competitive_match: ResMut<'w, CompetitiveMatchState>,
}
