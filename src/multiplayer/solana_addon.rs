//! Solana addon types for multiplayer integration
//!
//! Provides types for Solana wallet, game sync, and competitive match state.

use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::sync::Arc;

/// Result type for Solana operations
#[derive(Debug, Clone)]
pub enum SolanaResult<T> {
    Success(T),
    Error(String),
}

/// Solana wallet resource
#[derive(Resource, Debug, Clone, Default)]
pub struct SolanaWallet {
    pub pubkey: Option<Pubkey>,
    pub keypair: Option<Arc<solana_sdk::signature::Keypair>>,
    pub ranked_active: bool,
    pub tournament_match_id: Option<u64>,
}

impl SolanaWallet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_connected(&self) -> bool {
        self.pubkey.is_some()
    }
}

/// Game synchronization state with Solana
#[derive(Resource, Debug, Clone)]
pub struct SolanaGameSync {
    pub game_id: Option<u64>,
    pub moves_submitted: u32,
    pub wager_amount: u64,
    pub pending_confirmation: bool,
    pub last_signature: Option<Signature>,
    pub rpc_url: String,
    pub result_tx: Option<tokio::sync::mpsc::Sender<SolanaResult<Signature>>>,
}

impl Default for SolanaGameSync {
    fn default() -> Self {
        Self {
            game_id: None,
            moves_submitted: 0,
            wager_amount: 0,
            pending_confirmation: false,
            last_signature: None,
            rpc_url: "https://api.devnet.solana.com".to_string(),
            result_tx: None,
        }
    }
}

/// Competitive match state
#[derive(Resource, Debug, Clone, Default)]
pub struct CompetitiveMatchState {
    pub match_id: Option<u64>,
    pub opponent_pubkey: Option<Pubkey>,
    pub stake_amount: u64,
    pub is_ranked: bool,
    pub elo_rating: u32,
    pub opponent_elo: u32,
    pub active: bool,
    pub wager_lamports: u64,
    pub game_id: Option<u64>,
    pub finalizing_on_chain: bool,
    pub last_finalized_game_id: Option<u64>,
    pub last_error: Option<String>,
}

/// Player profile on Solana
#[derive(Resource, Debug, Clone, Default)]
pub struct SolanaProfile {
    pub username: String,
    pub elo: u32,
    pub games_played: u32,
    pub games_won: u32,
    pub total_wagered: u64,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
}
