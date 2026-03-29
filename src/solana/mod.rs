// Solana program integration module

// Sub-modules
pub mod core;
pub mod program_interface;
pub mod session;
pub mod wallet;
pub mod multiplayer;

// Re-export core items
pub use core::constants::*;
pub use core::errors::XfChessError;

// Re-export session items
pub use session::session::{GameRole, GameSession, SessionError, SessionPlugin, SessionState, check_session_validity};

// Re-export program_interface items
pub use program_interface::instructions::*;
pub use program_interface::state::{GameResult, GameStatus};

// Expose modules themselves so callers can use `crate::solana::instructions::Foo` paths
pub use core::errors;
pub use program_interface::instructions;
pub use program_interface::state;

// Re-export wallet items
pub use wallet::phantom_sign::{is_wallet_mode, phantom_sign};

// Re-export on-chain program types so the rest of the crate uses the
// canonical Anchor definitions rather than stale client-side mirrors.
pub use xfchess_game::state::game::{Game, GameResult as OnChainGameResult, GameStatus as OnChainGameStatus, GameType, SessionDelegation};
pub use xfchess_game::state::move_log::MoveLog;
pub use xfchess_game::state::player_profile::PlayerProfile;

use bevy::prelude::*;
use session::session::SessionPlugin as SessionPluginInner;

pub struct SolanaPlugin;

impl Plugin for SolanaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SessionPluginInner)
            .init_resource::<crate::multiplayer::solana::addon::SolanaWallet>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaGameSync>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaProfile>()
            .init_resource::<crate::multiplayer::solana::addon::CompetitiveMatchState>()
            .init_resource::<crate::multiplayer::solana::lobby::SolanaLobbyState>();
    }
}
