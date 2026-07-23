//! Multiplayer Module
//!
//! Provides P2P networking, VPS relay, and Solana integration for multiplayer chess.
//! This module is organized into specialized submodules for network, rollup, and solana logic.

use bevy::prelude::*;
use tokio::runtime::Runtime;

pub mod error;
pub mod join_link;
pub mod social;
pub mod systems;
pub mod telemetry;
pub mod traits;
pub mod types;

// Submodules
pub mod network;
#[cfg(feature = "solana")]
pub mod rollup;
#[cfg(feature = "solana")]
pub mod solana;
pub mod spectator;
pub mod tournament;
pub mod ui;
#[cfg(feature = "solana")]
pub mod wager_state;

/// Compatibility alias: the VPS HTTP client lives at `network::vps`. Older call
/// sites import it as `multiplayer::vps_client`.
pub mod vps_client {
    pub use super::network::vps::*;
}

// Re-exports for public API
pub use error::{MultiplayerError, MultiplayerResult};
pub use network::*;
#[cfg(feature = "solana")]
pub use rollup::*;
pub use traits::{Message, MessageReader, MessageWriter};
pub use types::*;

#[derive(Resource)]
pub struct TokioRuntime(pub Runtime);

/// Root plugin for all multiplayer functionality.
/// Orchestrates sub-plugins and registers core networking systems.
pub struct MultiplayerPlugin;

impl Plugin for MultiplayerPlugin {
    fn build(&self, app: &mut App) {
        // Initialize Tokio runtime for background tasks
        let runtime = Runtime::new().unwrap_or_else(|e| {
            panic!("Failed to create Tokio runtime: {}", e);
        });
        app.insert_resource(TokioRuntime(runtime));

        // 1. Register shared types and events
        app.init_resource::<OnlineNetworkState>()
            .init_resource::<OnlineGameSync>()
            .init_resource::<HeartbeatState>()
            .init_resource::<NetworkConfig>()
            .init_resource::<crate::multiplayer::types::CausalChainState>()
            .init_resource::<network::braid::BraidSubscriptionConfig>()
            .add_message::<NetworkEvent>()
            .add_message::<crate::game::events::GameStartedEvent>()
            .add_message::<crate::game::events::GameEndedEvent>();

        #[cfg(feature = "solana")]
        app.init_resource::<rollup::session_keys::SessionKeyManager>();

        // 2. Register sub-plugins
        app.add_plugins((
            network::p2p::P2PConnectionPlugin,
            network::p2p_vps::P2PVpsPlugin,
            network::online_game_session::OnlineGameSessionPlugin,
            network::relay_bridge::RelayBridgePlugin,
            social::SocialPlugin,
            join_link::JoinLinkPlugin,
            spectator::SpectatorPlugin,
            ui::spectator_overlay::SpectatorOverlayPlugin,
            telemetry::FocusTelemetryPlugin,
        ));

        #[cfg(feature = "solana")]
        app.add_plugins((
            rollup::manager::EphemeralRollupPlugin,
            rollup::bridge::RollupNetworkBridgePlugin,
            solana::integration::SolanaIntegrationPlugin,
            solana::lobby::SolanaLobbyPlugin,
            solana::wager_rate::SolUsdRatePlugin,
            // Backs ctx.tournament_client (ui/system_params/main_menu.rs) — the
            // resource every tournament register/join/expand click handler in
            // screens.rs is gated on. Without this, that Option resolves to
            // None at runtime and those buttons silently no-op.
            solana::tournament::TournamentClientPlugin,
        ));

        // 3. Register core orchestration systems
        app.add_systems(Startup, systems::initialize_braid_network)
            .add_systems(
                Update,
                (
                    systems::handle_network_events,
                    systems::dispatch_remote_moves,
                    systems::handle_resync_response,
                    systems::handle_resync_request,
                    systems::handle_game_control_messages,
                    systems::send_local_draw_events,
                    systems::tick_heartbeat,
                    systems::handle_pong,
                    systems::record_casual_game_on_end,
                ),
            );

        // 4. Register feature-specific cross-cutting systems
        #[cfg(feature = "solana")]
        {
            use crate::game::system_sets::GameSystems;
            app.add_systems(
                Update,
                (
                    // Step 1: feed moves into rollup batch AFTER GameSystems::Execution has applied them
                    (
                        systems::feed_local_moves_to_rollup,
                        systems::feed_remote_moves_to_rollup,
                    )
                        .after(GameSystems::Execution),
                    // Step 2: detect game over
                    systems::emit_game_ended_event.after(GameSystems::Execution),
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
