//! Unused constants scaffold — confirmed no caller anywhere in the codebase.
//! Stale: `PLAYER_SEED` below (`b"player"`) does not even match the real
//! on-chain profile seed (`b"profile"`, see `programs/xfchess-game/src/constants.rs`).
//! The real seeds and program ID used by live code are
//! `solana::program_interface::instructions`'s constants (re-exported as
//! `crate::solana::instructions::*`) and `solana-chess-client`.

use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

pub const SOLANA_PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

pub const GAME_SEED: &[u8] = b"game";
pub const PLAYER_SEED: &[u8] = b"player";

/// Default rent exemption amount for a 0-byte account (not sized for any real account).
pub const RENT_EXEMPTION_LAMPORTS: u64 = 890880;

pub const MAX_MOVES: usize = 200;

/// Timeout for games in seconds (10 minutes).
pub const GAME_TIMEOUT_SECONDS: i64 = 600;
