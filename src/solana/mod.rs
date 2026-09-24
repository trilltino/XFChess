// Solana program integration module

pub mod program_interface;
pub mod session;

// Keep the stable public path used by the client: `crate::solana::instructions::*`.
pub use program_interface::instructions;

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
