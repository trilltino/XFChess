use crate::core::{GameMode as CoreGameMode, GameState, PreviousState};
use crate::game::resources::{AIParams, GameStateParams};
use crate::game::resources::{CurrentTurn, GameTimer, MoveHistory, Players};
use crate::game::systems::input::InGameExitConfirmation;
use crate::game::view_mode::ViewMode;
#[cfg(feature = "solana")]
use crate::multiplayer::rollup::bridge::RecentTransactions;
#[cfg(feature = "solana")]
use crate::multiplayer::rollup::manager::EphemeralRollupManager;
#[cfg(feature = "solana")]
use crate::multiplayer::solana::addon::{
    CompetitiveMatchState, SolanaGameSync, SolanaProfile, SolanaWallet,
};
#[cfg(feature = "solana")]
use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
#[cfg(feature = "solana")]
use crate::ui::account::profile_view::ProfileViewState;
use crate::ui::game::game_ui::InGameHudVisibility;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

/// System parameter grouping UI-related resources
#[derive(SystemParam)]
pub struct GameUIParams<'w, 's> {
    pub contexts: EguiContexts<'w, 's>,
    pub game_state: GameStateParams<'w>,
    pub ai_params: AIParams<'w>,
    pub move_history: Res<'w, MoveHistory>,
    pub players: Res<'w, Players>,
    pub exit_confirmation: ResMut<'w, InGameExitConfirmation>,
    pub game_timer: Res<'w, GameTimer>,
    pub next_state: ResMut<'w, NextState<GameState>>,
    pub previous_state: ResMut<'w, PreviousState>,
    pub game_mode: Res<'w, CoreGameMode>,
    pub view_mode: ResMut<'w, ViewMode>,
    pub hud_visibility: Res<'w, InGameHudVisibility>,
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
    #[cfg(feature = "solana")]
    pub rollup_manager: Option<ResMut<'w, EphemeralRollupManager>>,
    #[cfg(feature = "solana")]
    pub solana_integration: Option<Res<'w, SolanaIntegrationState>>,
    #[cfg(feature = "solana")]
    pub profile_view: Option<ResMut<'w, ProfileViewState>>,
    #[cfg(feature = "solana")]
    pub global_session_active:
        Option<Res<'w, crate::multiplayer::solana::integration::systems::GlobalSessionActive>>,
    #[cfg(feature = "solana")]
    pub global_session_pending: Option<
        Res<'w, crate::multiplayer::solana::integration::systems::GlobalSessionCheckPending>,
    >,
    pub spectator_mode: Res<'w, crate::ui::spectator_mode::SpectatorMode>,
    pub active_time_control:
        Res<'w, crate::game::resources::active_time_control::ActiveTimeControl>,
    pub current_turn: Res<'w, CurrentTurn>,
    pub eval_history: Res<'w, crate::ui::game::game_2d::EvalHistory>,
    pub p2p_conn: Option<Res<'w, crate::multiplayer::network::p2p::P2PConnectionState>>,
    pub hourglass: Res<'w, crate::ui::game::game_ui::TimeoutHourglassState>,
    pub avatar_cache: ResMut<'w, crate::ui::game::game_ui::AvatarCache>,
    pub increment_flash: Res<'w, crate::ui::game::game_ui::IncrementFlash>,
    pub pending_draw: Res<'w, crate::game::systems::network_move::PendingDrawOffer>,
    pub turn_ctx: Res<'w, crate::game::resources::TurnStateContext>,
    pub resign_writer: bevy::prelude::MessageWriter<'w, crate::game::events::ResignEvent>,
    pub draw_writer: bevy::prelude::MessageWriter<'w, crate::game::events::DrawOfferEvent>,
    pub first_move_deadline: Res<'w, crate::game::resources::FirstMoveDeadline>,
    pub chat_state: ResMut<'w, crate::ui::game::chat_ui::ChatState>,
    pub chat_writer:
        bevy::prelude::MessageWriter<'w, crate::multiplayer::network::PublishOnlineChat>,
    pub player_identity: Option<Res<'w, crate::states::main_menu::PlayerIdentity>>,
}
