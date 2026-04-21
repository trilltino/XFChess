//! Multiplayer Module
//!
//! Provides P2P networking, VPS relay, and Solana integration for multiplayer chess.
//! This module is organized into specialized submodules for network, rollup, and solana logic.

use bevy::prelude::*;
use tokio::runtime::Runtime;

pub mod error;
pub mod traits;
pub mod types;
pub mod systems;

// Submodules
pub mod network;
#[cfg(feature = "solana")]
pub mod solana;
#[cfg(feature = "solana")]
pub mod rollup;
pub mod spectator;
pub mod tournament;
pub mod turn_relay;
pub mod ui;
pub mod vps_client;
#[cfg(feature = "solana")]
pub mod wager_state;

// Re-exports for public API
pub use error::{MultiplayerError, MultiplayerResult};
pub use traits::{AddMessage, Message, MessageReader, MessageWriter};
pub use types::*;
pub use network::*;
#[cfg(feature = "solana")]
pub use rollup::*;

#[derive(Resource)]
pub struct TokioRuntime(pub Runtime);

/// Root plugin for all multiplayer functionality.
/// Orchestrates sub-plugins and registers core networking systems.
pub struct MultiplayerPlugin;

impl Plugin for MultiplayerPlugin {
    fn build(&self, app: &mut App) {
        // 1. Register shared types and events
        app.init_resource::<BraidNetworkState>()
            .init_resource::<BraidGameSync>()
            .init_resource::<network::braid::BraidP2PConfig>()
            .add_message::<NetworkEvent>()
            .add_message::<crate::game::events::GameStartedEvent>()
            .add_message::<crate::game::events::GameEndedEvent>();

        #[cfg(feature = "solana")]
        app.init_resource::<rollup::session_keys::SessionKeyManager>();

        // 2. Register sub-plugins
        app.add_plugins((
            network::p2p::P2PConnectionPlugin,
            network::p2p_vps::P2PVpsPlugin,
            spectator::SpectatorPlugin,
        ));

        #[cfg(feature = "solana")]
        app.add_plugins((
            rollup::manager::EphemeralRollupPlugin,
            rollup::bridge::RollupNetworkBridgePlugin,
            solana::integration::SolanaIntegrationPlugin,
            solana::lobby::SolanaLobbyPlugin,
        ));

        // 3. Register core orchestration systems
        app.add_systems(Startup, systems::initialize_braid_network)
            .add_systems(Update, systems::handle_network_events);

        // 4. Register feature-specific cross-cutting systems
        #[cfg(feature = "solana")]
        {
            use crate::game::system_sets::GameSystems;
            app.add_systems(
                Update,
                (
                    // Step 1: feed moves into rollup batch AFTER GameSystems::Execution has applied them
                    (systems::feed_local_moves_to_rollup, systems::feed_remote_moves_to_rollup)
                        .after(GameSystems::Execution),
                    // Step 2: detect game over
                    systems::emit_game_ended_event
                        .after(GameSystems::Execution),
                    // Step 3: flush batch AFTER feeds have added all moves and event is emitted
                    systems::finalize_game_on_end
                        .after(systems::feed_local_moves_to_rollup)
                        .after(systems::feed_remote_moves_to_rollup)
                        .after(systems::emit_game_ended_event),
                    systems::handle_session_info_from_network,
                ),
            );
        }
    }
}
