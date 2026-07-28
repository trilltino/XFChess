//! Client-side Rust bindings for the `xfchess-game` Solana program: PDA
//! derivation, account fetching, and instruction builders. Linked into the
//! game client behind `--features solana` and used by the backend's signing
//! routes to build unsigned transactions. See the crate README for the full
//! builder inventory.

pub mod rpc;
pub mod wallet;

// Re-export core types
pub use rpc::{ChessRpcClient, Error};
pub use wallet::{KeypairWallet, Wallet};

/// Deployed program ID (localnet + devnet) — matches `declare_id!` in `programs/xfchess-game`.
pub const XFCHESS_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

/// PDA Seeds matching Anchor
pub const GAME_SEED: &[u8] = b"game";
/// Seed for a game's move-log PDA.
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
/// Seed for a player's profile PDA.
pub const PROFILE_SEED: &[u8] = b"profile";
