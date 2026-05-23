//! Networking submodules for XFChess multiplayer.
//!
//! This module groups the client-side network layers:
//! - `braid` / `braid_pvp` — iroh/Braid P2P gossip transport for live games
//! - `p2p` / `p2p_vps` — Bevy-facing P2P connection state and VPS relay polling
//! - `protocol` — shared wire-format message types
//! - `vps` — blocking HTTP client for the XFChess signing-server VPS
//! - `relay` — STUN/TURN style relay helpers
//! - `game_id_store` — caches mapping between on-chain game IDs and sessions
//!
//! Re-exports the most commonly used items so callers can depend on
//! `crate::multiplayer::network::*` without reaching into each submodule.

pub mod braid;
pub mod braid_pvp;
pub mod p2p;
pub mod p2p_vps;
pub mod protocol;
pub mod game_id_store;
pub mod vps;
pub mod relay;

pub use braid::*;
pub use braid_pvp::{
    BraidPvpIncomingMessage, BraidPvpPlugin, BraidPvpSession, PublishBraidMove,
    PublishBraidResign, PublishBraidChat,
};
pub use p2p::*;
pub use protocol::*;
pub use vps::*;
pub use relay::*;
