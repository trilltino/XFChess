//! Wallet and session-key state for on-chain play, kept in sync with the wallet
//! bridge and the backend.

pub mod plugin;
pub mod profile_check;
pub mod state;
pub mod systems;

pub use plugin::SolanaIntegrationPlugin;
pub use state::SolanaIntegrationState;
