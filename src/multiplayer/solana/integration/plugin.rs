use bevy::prelude::*;

use super::profile_check::{check_profile_on_connect, handle_profile_check_tasks};
use super::state::{BalanceRefreshTimer, SolanaIntegrationState};
use super::systems::*;
use crate::ui::account::profile_view::{
    fetch_profile_history, poll_profile_history, profile_view_ui, ProfileViewState,
};

pub struct SolanaIntegrationPlugin;

impl Plugin for SolanaIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaIntegrationState>();
        app.init_resource::<BalanceRefreshTimer>();
        app.init_resource::<ProfileViewState>();
        app.add_systems(Update, initialize_solana_integration);
        app.add_systems(Update, update_wallet_balance);
        app.add_systems(Update, update_wallet_usd_rate);
        app.add_systems(Update, handle_pending_solana_tasks);
        app.add_systems(Update, sync_session_key_to_network);
        app.add_systems(Update, authorize_session_key_on_game_start);
        app.add_systems(Update, poll_session_pubkey_update);
        app.add_systems(Update, spawn_verified_participants_fetch);
        app.add_systems(Update, poll_verified_participants_fetch);
        app.add_systems(
            OnEnter(crate::core::states::MenuState::Main),
            verify_global_session_on_menu_enter,
        );
        app.add_systems(Update, poll_global_session_result);
        app.add_systems(Update, poll_global_session_register_result);
        // Opt in with XFCHESS_ENABLE_GLOBAL_SESSION_AUTOSETUP=1. Automatic setup may
        // require a deposit and revoke/reauthorize signatures; per-game signing remains
        // available when no global session is cached.
        if std::env::var("XFCHESS_ENABLE_GLOBAL_SESSION_AUTOSETUP").is_ok_and(|v| v == "1") {
            app.add_systems(Update, authorize_global_session_if_needed);
        }
        app.add_systems(Update, check_profile_on_connect);
        app.add_systems(Update, handle_profile_check_tasks);
        app.add_systems(Update, fetch_user_status_async);
        app.add_systems(Update, sync_player_profiles);

        // Profile view overlay
        app.add_systems(
            Update,
            (fetch_profile_history, poll_profile_history, profile_view_ui),
        );
    }
}
