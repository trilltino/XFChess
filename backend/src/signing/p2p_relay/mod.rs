//! P2P game relay for VPS-mediated peer-to-peer connections.
//!
//! This module provides a VPS-mediated relay system for P2P chess games,
//! enabling reliable NAT traversal for players behind firewalls.
//!
//! Features:
//! - Game announcement and discovery
//! - Player matchmaking with node ID exchange
//! - In-game message relay for move data
//! - Automatic cleanup of stale games

pub mod routes;
pub mod state;
pub mod types;

pub use routes::p2p_routes;
pub use state::{create_relay_state, P2PRelayState};
pub use types::{ActiveGame, GameListing, GameStatus, P2PGameAnnouncement};
