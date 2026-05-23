// Solana program integration module

// Sub-modules
pub mod core;
pub mod program_interface;
pub mod session;
pub mod wallet;
pub mod multiplayer;

// Expose modules themselves so callers can use `crate::solana::instructions::Foo` paths
pub use program_interface::instructions;

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
