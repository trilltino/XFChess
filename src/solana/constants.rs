// Solana program constants
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

// Program ID for our Solana program
pub const SOLANA_PROGRAM_ID: &str = "3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP";

// Seeds for PDAs
pub const GAME_SEED: &[u8] = b"game";
pub const PLAYER_SEED: &[u8] = b"player";

// Default rent exemption amount
pub const RENT_EXEMPTION_LAMPORTS: u64 = 890880; // Amount for 0 bytes data (adjust based on your data size)

// Maximum number of moves in a game
pub const MAX_MOVES: usize = 200;

// Timeout for games in seconds
pub const GAME_TIMEOUT_SECONDS: i64 = 600; // 10 minutes
