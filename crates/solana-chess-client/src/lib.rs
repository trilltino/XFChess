pub mod rpc;
pub mod wallet;

// Re-export core types
pub use rpc::{ChessRpcClient, Error};
pub use wallet::{KeypairWallet, Wallet};

// Program ID (matches the one in xfchess-game)
pub const XFCHESS_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

/// PDA Seeds matching Anchor
pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const PROFILE_SEED: &[u8] = b"profile";
