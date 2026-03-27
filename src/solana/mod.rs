// Solana program integration module
pub mod errors;
pub mod instructions;
pub mod multiplayer;
pub mod phantom_sign;
pub mod session;
mod state;

// Re-export on-chain program types so the rest of the crate uses the
// canonical Anchor definitions rather than stale client-side mirrors.
pub use xfchess_game::state::game::{Game, GameResult, GameStatus, GameType, SessionDelegation};
pub use xfchess_game::state::move_log::MoveLog;
pub use xfchess_game::state::player_profile::PlayerProfile;

use bevy::prelude::*;
use session::SessionPlugin;

pub struct SolanaPlugin;

impl Plugin for SolanaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SessionPlugin)
            .init_resource::<crate::multiplayer::solana::addon::SolanaWallet>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaGameSync>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaProfile>()
            .init_resource::<crate::multiplayer::solana::addon::CompetitiveMatchState>()
            .init_resource::<crate::multiplayer::solana::lobby::SolanaLobbyState>();
    }
}
