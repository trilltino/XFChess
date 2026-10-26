//! Transports and identities: the P2P mesh protocol and reorder buffer, the Braid
//! move transport, VPS relay lobbies, the backend HTTP client ([`vps`]), and the
//! local node and device identities.

pub mod braid_transport;
pub mod device_id;
pub mod game_id_store;
pub mod identity;
pub mod move_auth;
pub mod online_game_session;
pub mod p2p;
pub mod p2p_vps;
pub mod protocol;
pub mod reorder;
pub mod vps;

pub use online_game_session::{
    OnlineChatMessage, OnlineGameSession, OnlineGameSessionPlugin, PublishOnlineChat,
};
pub use p2p::*;
pub use protocol::*;
pub use reorder::{IngestOutcome, NonceSequencer};
pub use vps::*;
