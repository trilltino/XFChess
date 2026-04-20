use crate::core::{GameMode as CoreGameMode, GameState, PreviousState};
use crate::game::resources::{AIParams, GameStateParams};
use crate::game::resources::{GameTimer, MoveHistory, Players};
use crate::game::view_mode::{PlayerViewPreferences, ViewMode};
#[cfg(feature = "solana")]
use crate::multiplayer::solana::addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
#[cfg(feature = "solana")]
use crate::multiplayer::rollup::bridge::RecentTransactions;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

/// System parameter grouping UI-related resources
#[derive(SystemParam)]
pub struct GameUIParams<'w, 's> {
    pub contexts: EguiContexts<'w, 's>,
    pub game_state: GameStateParams<'w>,
    #[allow(dead_code)]
    pub ai_params: AIParams<'w>,
    pub move_history: Res<'w, MoveHistory>,
    #[allow(dead_code)]
    pub players: Res<'w, Players>,
    pub game_timer: Res<'w, GameTimer>,
    pub next_state: ResMut<'w, NextState<GameState>>,
    #[allow(dead_code)]
    pub previous_state: ResMut<'w, PreviousState>,
    pub game_mode: Res<'w, CoreGameMode>,
    pub view_preferences: ResMut<'w, PlayerViewPreferences>,
    pub view_mode: ResMut<'w, ViewMode>,
    #[cfg(feature = "solana")]
    pub solana_wallet: Option<ResMut<'w, SolanaWallet>>,
    #[cfg(feature = "solana")]
    pub solana_sync: Option<ResMut<'w, SolanaGameSync>>,
    #[cfg(feature = "solana")]
    pub solana_profile: Option<Res<'w, SolanaProfile>>,
    #[cfg(feature = "solana")]
    pub competitive_match: Option<ResMut<'w, CompetitiveMatchState>>,
    #[cfg(feature = "solana")]
    pub recent_txs: Option<Res<'w, RecentTransactions>>,
}
