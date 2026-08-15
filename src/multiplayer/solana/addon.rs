//! Solana addon types for multiplayer integration
//!
//! Provides types for Solana wallet, game sync, and competitive match state.

use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::sync::Arc;

use crate::multiplayer::vps_client::UserStatus;

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
    /// Cached verification status from VPS backend
    pub user_status: Option<UserStatus>,
}

impl SolanaWallet {
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
    /// True only for games whose move flow genuinely depends on MagicBlock
    /// Ephemeral-Rollup delegation: wagered lobby games, tournament games,
    /// and rejoins of either. Move input is gated on delegation completing
    /// (`can_move_color` in `game/systems/input.rs`) *only* when this is set.
    ///
    /// Pre-v0.2.8 the gate keyed off `game_id.is_some()` instead — which
    /// permanently locked White in stake-0 free lobby games (`game_id` set,
    /// but no delegation ever completes for them) and in pure casual P2P
    /// games that inherited a stale `game_id` from an earlier Solana game.
    pub requires_delegation: bool,
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
            requires_delegation: false,
        }
    }
}

/// Clears any on-chain game context (`SolanaGameSync` +
/// `CompetitiveMatchState`) when entering a pure casual P2P game.
///
/// Without this, leftover `game_id`/`active` from a previous Solana lobby or
/// tournament match leaks into the casual game: the input gate would block
/// White forever waiting for a delegation a casual game never gets, and the
/// Braid publisher would send the wallet pubkey as its identity instead of
/// the Iroh node id the backend's casual relay roster actually knows (HTTP
/// 403) — the two failure modes that broke free P2P games in v0.2.7.
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

/// Competitive match state
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

/// Player profile on Solana
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
    /// Total games played, computed from wins + losses + draws.
    pub fn games_played(&self) -> u32 {
        self.wins + self.losses + self.draws
    }
}
