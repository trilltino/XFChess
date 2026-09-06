//! Client networking for online games, Braid transport, P2P, and VPS calls.

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
