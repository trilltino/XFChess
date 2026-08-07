//! Networking submodules for XFChess multiplayer.
//!
//! This module groups the client-side network layers:
//! - `online_game_session` - live game transport over Iroh gossip plus the Braid moves/chat log
//! - `braid_transport` - durable, push-based Braid moves/resign/chat transport (replaced the old move-relay use of `p2p_relay`)
//! - `p2p` / `p2p_vps` - Bevy-facing peer state and relay-backed lobby polling (JOIN_ACK handshake — still backend `p2p_relay`, unrelated to move sync)
//! - `protocol` - shared wire-format message types
//! - `vps` - blocking HTTP client for the XFChess signing-server VPS
//! - `reorder` - per-game nonce reordering buffer for the dual (gossip + Braid) transport
//! - `game_id_store` - caches mapping between on-chain game IDs and sessions
//!
//! Two modules used to be listed here that were never on the move path, and
//! have been removed rather than left to imply a fallback that didn't exist:
//! `relay` (a self-contained STUN/TURN client with zero call sites — Iroh does
//! its own NAT traversal and relaying) and `braid` (a legacy document-
//! subscription state whose types nothing referenced; the live Braid path is
//! `braid_transport`).
//!
//! Re-exports the most commonly used items so callers can depend on
//! `crate::multiplayer::network::*` without reaching into each submodule.

pub mod braid_transport;
pub mod game_id_store;
pub mod identity;
pub mod online_game_session;
pub mod p2p;
pub mod p2p_vps;
pub mod protocol;
pub mod reorder;
pub mod vps;

pub use online_game_session::{
    OnlineChatMessage, OnlineGameSession, OnlineGameSessionPlugin, PublishOnlineChat,
    PublishOnlineResign,
};
pub use p2p::*;
pub use protocol::*;
pub use reorder::{IngestOutcome, NonceSequencer};
pub use vps::*;
