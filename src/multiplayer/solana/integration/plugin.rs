use bevy::prelude::*;

use crate::core::schedule::SessionSet;

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
        // Wallet connection and balance, profiles, the sign-once global session,
        // then per-game session keys — each step sees the previous one's result.
        app.add_systems(
            Update,
            (
                initialize_solana_integration,
                update_wallet_balance,
                update_wallet_usd_rate,
                handle_pending_solana_tasks,
                check_profile_on_connect,
                handle_profile_check_tasks,
                fetch_user_status_async,
                sync_player_profiles,
                poll_global_session_result,
                poll_global_session_register_result,
                sync_session_key_to_network,
                authorize_session_key_on_game_start,
                poll_session_pubkey_update,
                spawn_verified_participants_fetch,
                poll_verified_participants_fetch,
                fetch_profile_history,
                poll_profile_history,
            )
                .chain()
                .in_set(SessionSet::Wallet),
        );
        app.add_systems(
            OnEnter(crate::core::states::MenuState::Main),
            verify_global_session_on_menu_enter,
        );
        // Opt in with XFCHESS_ENABLE_GLOBAL_SESSION_AUTOSETUP=1. Automatic setup may
        // require a deposit and revoke/reauthorize signatures; per-game signing remains
        // available when no global session is cached.
        if std::env::var("XFCHESS_ENABLE_GLOBAL_SESSION_AUTOSETUP").is_ok_and(|v| v == "1") {
            app.add_systems(
                Update,
                authorize_global_session_if_needed
                    .after(poll_global_session_register_result)
                    .before(sync_session_key_to_network)
                    .in_set(SessionSet::Wallet),
            );
        }
        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            profile_view_ui.run_if(|view: Res<ProfileViewState>| view.open),
        );
    }
}
