//! Online play: backend auth, the Iroh/Braid P2P transport, game sessions,
//! spectating, resume, and (with `--features solana`) wagers, rollups and
//! tournaments. [`MultiplayerPlugin`] owns the shared Tokio runtime and registers
//! the sub-plugins. See [README.md](README.md).

use bevy::prelude::*;
use tokio::runtime::Runtime;

pub mod casual_resume;
pub mod error;
pub mod join_link;
pub mod monitoring;
pub mod social;
pub mod systems;
pub mod telemetry;
pub mod traits;
pub mod types;

pub mod network;
#[cfg(feature = "solana")]
pub mod resume;
#[cfg(feature = "solana")]
pub mod rollup;
#[cfg(feature = "solana")]
pub mod seat;
pub mod session_restore;
#[cfg(feature = "solana")]
pub mod solana;
pub mod spectator;
#[cfg(test)]
mod start_handshake_sim;
pub mod tournament;
pub mod ui;
#[cfg(feature = "solana")]
pub mod wager_state;

pub mod vps_client {
    pub use super::network::vps::*;
}

pub use error::{MultiplayerError, MultiplayerResult};
pub use network::*;
#[cfg(feature = "solana")]
pub use rollup::*;
pub use traits::{Message, MessageReader, MessageWriter};
pub use types::*;

#[derive(Resource)]
pub struct TokioRuntime(pub Runtime);

pub struct MultiplayerPlugin;

impl Plugin for MultiplayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineNetworkState>()
            .init_resource::<OnlineGameSync>()
            .init_resource::<HeartbeatState>()
            .init_resource::<NetworkConfig>()
            .init_resource::<crate::multiplayer::types::CausalChainState>()
            .init_resource::<crate::multiplayer::types::PendingMoveBuffer>()
            .add_message::<NetworkEvent>()
            .add_message::<crate::game::events::GameStartedEvent>()
            .add_message::<crate::game::events::GameEndedEvent>();

        #[cfg(feature = "solana")]
        app.init_resource::<rollup::session_keys::HandshakeOrderingKeyManager>();

        app.add_plugins((
            network::p2p::P2PConnectionPlugin,
            network::p2p_vps::P2PVpsPlugin,
            network::online_game_session::OnlineGameSessionPlugin,
            network::braid_transport::BraidTransportPlugin,
            social::SocialPlugin,
            join_link::JoinLinkPlugin,
            spectator::SpectatorPlugin,
            ui::spectator_overlay::SpectatorOverlayPlugin,
            telemetry::FocusTelemetryPlugin,
            monitoring::GameHealthMonitorPlugin,
            session_restore::SessionRestorePlugin,
            casual_resume::CasualResumePlugin,
        ));

        #[cfg(feature = "solana")]
        app.add_plugins((
            rollup::manager::EphemeralRollupPlugin,
            rollup::bridge::RollupNetworkBridgePlugin,
            solana::integration::SolanaIntegrationPlugin,
            solana::lobby::SolanaLobbyPlugin,
            solana::chain_actions::ChainActionsPlugin,
            solana::wager_rate::SolUsdRatePlugin,
            // Tournament click handlers require this resource; a missing client silently disables them.
            solana::tournament::TournamentClientPlugin,
            monitoring::RollupHealthMonitorPlugin,
            resume::ResumePlugin,
            seat::SeatPlugin,
        ));

        app.add_systems(
            Update,
            (
                systems::initialize_braid_network
                    .in_set(crate::core::schedule::ReceiveSet::Transport),
                (
                    systems::handle_network_events,
                    systems::sweep_stale_move_buffers,
                    systems::dispatch_remote_moves,
                    systems::handle_resync_response,
                    systems::poll_trusted_recovery,
                    systems::handle_resync_request,
                    systems::handle_game_control_messages,
                    systems::handle_pong,
                )
                    .chain()
                    .in_set(crate::core::schedule::ReceiveSet::Game),
                (
                    systems::send_local_draw_events,
                    // Gate heartbeats to InGame; game_mode persists after returning to the menu.
                    systems::tick_heartbeat.run_if(in_state(crate::core::state::GameState::InGame)),
                    systems::record_casual_game_on_end,
                )
                    .chain()
                    .in_set(crate::core::schedule::SendSet::Session),
            ),
        )
        .add_systems(
            OnExit(crate::core::state::GameState::InGame),
            systems::reset_multiplayer_session_state,
        );

        #[cfg(feature = "solana")]
        {
            app.add_systems(
                Update,
                (
                    (
                        systems::feed_local_moves_to_rollup,
                        systems::feed_remote_moves_to_rollup,
                        systems::emit_game_ended_event,
                        systems::finalize_game_on_end,
                    )
                        .chain()
                        .in_set(crate::core::schedule::SendSet::RollupFeed),
                    systems::handle_session_info_from_network
                        .after(systems::handle_pong)
                        .in_set(crate::core::schedule::ReceiveSet::Game),
                ),
            );
        }
    }
}
