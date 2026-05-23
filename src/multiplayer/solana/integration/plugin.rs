use bevy::prelude::*;

use super::profile_check::{check_profile_on_connect, handle_profile_check_tasks};
use super::state::{SolanaIntegrationState, BalanceRefreshTimer};
use super::systems::*;
use crate::ui::profile_creation::{
    despawn_profile_creation_ui, handle_profile_submission, profile_creation_ui_system, 
    spawn_profile_creation_ui, validate_username_system, ProfileCreationState, ProfileSubmissionEvent
};
use crate::core::states::MenuState;

// Plugin for Solana integration
pub struct SolanaIntegrationPlugin;

impl Plugin for SolanaIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaIntegrationState>();
        app.init_resource::<BalanceRefreshTimer>();
        app.init_resource::<ProfileCreationState>();
        app.add_message::<ProfileSubmissionEvent>();
        app.add_systems(Update, initialize_solana_integration);
        app.add_systems(Update, update_wallet_balance);
        app.add_systems(Update, update_wallet_usd_rate);
        app.add_systems(Update, handle_pending_solana_tasks);
        app.add_systems(Update, monitor_network_handshakes);
        app.add_systems(Update, sync_session_key_to_network);
        app.add_systems(Update, authorize_session_key_on_game_start);
        app.add_systems(Update, check_session_expiry_on_game_start);
        app.add_systems(
            OnEnter(crate::core::states::MenuState::Main),
            verify_global_session_on_menu_enter,
        );
        app.add_systems(Update, check_profile_on_connect);
        app.add_systems(Update, handle_profile_check_tasks);
        app.add_systems(Update, fetch_user_status_async);
        app.add_systems(Update, sync_player_profiles);
        
        // Profile creation UI systems
        app.add_systems(OnEnter(MenuState::ProfileCreation), spawn_profile_creation_ui);
        app.add_systems(Update, (profile_creation_ui_system).run_if(in_state(MenuState::ProfileCreation)));
        app.add_systems(Update, (validate_username_system).run_if(in_state(MenuState::ProfileCreation)));
        app.add_systems(Update, handle_profile_submission);
        app.add_systems(OnExit(MenuState::ProfileCreation), despawn_profile_creation_ui);
    }
}
