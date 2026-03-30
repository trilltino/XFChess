use bevy::prelude::*;

use super::state::{SolanaIntegrationState, BalanceRefreshTimer};
use super::systems::*;

// Plugin for Solana integration
pub struct SolanaIntegrationPlugin;

impl Plugin for SolanaIntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaIntegrationState>();
        app.init_resource::<BalanceRefreshTimer>();
        app.add_systems(Update, initialize_solana_integration);
        app.add_systems(Update, update_wallet_balance);
        app.add_systems(Update, handle_pending_solana_tasks);
        app.add_systems(Update, monitor_network_handshakes);
        app.add_systems(Update, authorize_session_key_on_game_start);
    }
}
