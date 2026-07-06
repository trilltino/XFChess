//! Networking submodules for XFChess multiplayer.
//!
//! This module groups the client-side network layers:
//! - `online_game_session` - live game transport over Iroh gossip plus VPS relay mirroring
//! - `braid` - legacy Braid document subscription state
//! - `p2p` / `p2p_vps` - Bevy-facing peer state and relay-backed lobby polling
//! - `protocol` - shared wire-format message types
//! - `vps` - blocking HTTP client for the XFChess signing-server VPS
//! - `relay` - STUN/TURN style relay helpers
//! - `game_id_store` - caches mapping between on-chain game IDs and sessions
//!
//! Re-exports the most commonly used items so callers can depend on
//! `crate::multiplayer::network::*` without reaching into each submodule.

pub mod braid;
pub mod game_id_store;
pub mod identity;
pub mod online_game_session;
pub mod p2p;
pub mod p2p_vps;
pub mod protocol;
pub mod relay;
pub mod relay_bridge;
pub mod vps;

pub use braid::*;
pub use online_game_session::{
    OnlineChatMessage, OnlineGameSession, OnlineGameSessionPlugin, PublishOnlineChat,
    PublishOnlineResign,
};
pub use p2p::*;
pub use protocol::*;
pub use relay::*;
pub use vps::*;
