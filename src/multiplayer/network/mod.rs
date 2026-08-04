//! Networking submodules for XFChess multiplayer.
//!
//! This module groups the client-side network layers:
//! - `online_game_session` - live game transport over Iroh gossip plus the Braid moves/chat log
//! - `braid_transport` - durable, push-based Braid moves/resign/chat transport (replaced the old move-relay use of `p2p_relay`)
//! - `braid` - legacy Braid document subscription state
//! - `p2p` / `p2p_vps` - Bevy-facing peer state and relay-backed lobby polling (JOIN_ACK handshake — still backend `p2p_relay`, unrelated to move sync)
//! - `protocol` - shared wire-format message types
//! - `vps` - blocking HTTP client for the XFChess signing-server VPS
//! - `relay` - STUN/TURN style relay helpers
//! - `reorder` - per-game nonce reordering buffer for the dual (gossip + Braid) transport
//! - `game_id_store` - caches mapping between on-chain game IDs and sessions
//!
//! Re-exports the most commonly used items so callers can depend on
//! `crate::multiplayer::network::*` without reaching into each submodule.

pub mod braid;
pub mod braid_transport;
pub mod game_id_store;
pub mod identity;
pub mod online_game_session;
pub mod p2p;
pub mod p2p_vps;
pub mod protocol;
pub mod relay;
pub mod reorder;
pub mod vps;

pub use braid::*;
pub use online_game_session::{
    OnlineChatMessage, OnlineGameSession, OnlineGameSessionPlugin, PublishOnlineChat,
    PublishOnlineResign,
};
pub use p2p::*;
pub use protocol::*;
pub use relay::*;
pub use reorder::{IngestOutcome, NonceSequencer};
pub use vps::*;
