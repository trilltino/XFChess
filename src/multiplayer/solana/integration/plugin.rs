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
        app.add_systems(Update, poll_session_pubkey_update);
        app.add_systems(Update, spawn_verified_participants_fetch);
        app.add_systems(Update, poll_verified_participants_fetch);
        app.add_systems(
            OnEnter(crate::core::states::MenuState::Main),
            verify_global_session_on_menu_enter,
        );
        app.add_systems(Update, poll_global_session_result);
        app.add_systems(Update, poll_global_session_register_result);
        // Was disabled outright by request: auto-establishing a new global
        // session was the single biggest source of confusing popups
        // (revoke+reauthorize cascades, a real 0.11 SOL deposit, 3
        // timed-out wallet-approval attempts that still fell back to
        // per-game signing anyway). It is NOT required for gameplay —
        // lobby.rs already falls back to per-game signing whenever no
        // global session is cached, which is the path every game (free or
        // wagered) already works through. `try_load_global_session` (called
        // at wallet-connect, in `initialize_solana_integration`) still
        // opportunistically picks up an existing, correctly-matched session
        // regardless of this flag.
        //
        // Since then: `establish_global_session` gained a pre-flight
        // on-chain check (`global_session_is_live_onchain`) that skips the
        // doomed first authorize attempt entirely when a stale delegation is
        // already known, cutting the worst-case cascade from "attempt, fail,
        // revoke, reauthorize" down to "revoke, reauthorize" — and the
        // Tauri wallet bridge now hides (reuses) the popup between
        // signatures instead of killing and respawning a whole browser
        // process for each one, so a 2-signature revoke+reauth cascade is
        // far cheaper than when this was disabled. Still gated behind an
        // opt-in env var rather than flipped on by default: this exact path
        // has never been exercised against a live app end-to-end (see
        // docs/plans/global-session-flow-fix-plan.md), only `cargo check`.
        // Set XFCHESS_ENABLE_GLOBAL_SESSION_AUTOSETUP=1 to opt in for
        // testing; flip the default once that's happened cleanly.
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
