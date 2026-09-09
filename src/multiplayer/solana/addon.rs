use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::sync::Arc;

use crate::multiplayer::vps_client::UserStatus;

#[derive(Debug, Clone)]
pub enum SolanaResult<T> {
    Success(T),
    Error(String),
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SolanaWallet {
    pub pubkey: Option<Pubkey>,
    pub keypair: Option<Arc<solana_sdk::signature::Keypair>>,
    pub ranked_active: bool,
    pub tournament_match_id: Option<u64>,
    pub user_status: Option<UserStatus>,
}

impl SolanaWallet {
    pub fn is_connected(&self) -> bool {
        self.pubkey.is_some()
    }
}

#[derive(Resource, Debug, Clone)]
pub struct SolanaGameSync {
    pub game_id: Option<u64>,
    pub session_pubkey: Option<Pubkey>,
    pub session_pubkey_update: std::sync::Arc<std::sync::Mutex<Option<Pubkey>>>,
    pub moves_submitted: u32,
    pub wager_amount: u64,
    pub pending_confirmation: bool,
    pub last_signature: Option<Signature>,
    pub rpc_url: String,
    pub result_tx: Option<tokio::sync::mpsc::Sender<SolanaResult<Signature>>>,
    pub requires_delegation: bool,
}

impl Default for SolanaGameSync {
    fn default() -> Self {
        Self {
            game_id: None,
            session_pubkey: None,
            session_pubkey_update: std::sync::Arc::new(std::sync::Mutex::new(None)),
            moves_submitted: 0,
            wager_amount: 0,
            pending_confirmation: false,
            last_signature: None,
            rpc_url: "https://api.devnet.solana.com".to_string(),
            result_tx: None,
            requires_delegation: false,
        }
    }
}

pub fn clear_on_chain_game_state(
    sync: Option<&mut SolanaGameSync>,
    competitive: Option<&mut CompetitiveMatchState>,
) {
    if let Some(sync) = sync {
        *sync = SolanaGameSync::default();
    }
    if let Some(competitive) = competitive {
        *competitive = CompetitiveMatchState::default();
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct CompetitiveMatchState {
    pub match_id: Option<u64>,
    pub opponent_pubkey: Option<Pubkey>,
    pub stake_amount: u64,
    pub is_ranked: bool,
    pub elo_rating: u32,
    pub opponent_elo: u32,
    pub opponent_username: String,
    pub opponent_country: String,
    pub active: bool,
    pub wager_lamports: u64,
    pub game_id: Option<u64>,
    pub finalizing_on_chain: bool,
    pub last_finalized_game_id: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SolanaProfile {
    pub username: String,
    pub country: String,
    pub elo: u32,
    pub total_wagered: u64,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub is_verified: bool,
}

impl SolanaProfile {
    pub fn games_played(&self) -> u32 {
        self.wins + self.losses + self.draws
    }
}
