use bevy::prelude::*;

use super::profile_check::{check_profile_on_connect, handle_profile_check_tasks};
use super::state::{BalanceRefreshTimer, SolanaIntegrationState};
use super::systems::*;
use crate::ui::account::profile_view::{
    fetch_profile_history, poll_profile_history, profile_view_ui, ProfileViewState,
};

// Plugin for Solana integration
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
        app.add_systems(
            OnEnter(crate::core::states::MenuState::Main),
            verify_global_session_on_menu_enter,
        );
        app.add_systems(Update, poll_global_session_result);
        app.add_systems(Update, poll_global_session_register_result);
        // Disabled by request: auto-establishing a new global session was the
        // single biggest source of confusing popups (revoke+reauthorize
        // cascades, a real 0.11 SOL deposit, 3 timed-out wallet-approval
        // attempts that still fell back to per-game signing anyway). It is
        // NOT required for gameplay — lobby.rs already falls back to
        // per-game signing whenever no global session is cached, which is
        // the path every game (free or wagered) already works through.
        // `try_load_global_session` (called at wallet-connect, in
        // `initialize_solana_integration`) still opportunistically picks up
        // an existing, correctly-matched session with zero code changes
        // needed — this line only stops the client from trying to establish
        // a *new* one automatically. Re-enable by uncommenting once the
        // wallet-approval-popup timeout/focus issue has been diagnosed with
        // a live repro.
        // app.add_systems(Update, authorize_global_session_if_needed);
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
