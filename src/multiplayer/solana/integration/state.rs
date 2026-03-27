use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};

use crate::solana::instructions::{
    GAME_SEED, MOVE_LOG_SEED, PROFILE_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
    SESSION_DELEGATION_SEED, WAGER_ESCROW_SEED,
};

/// Devnet RPC endpoint
pub const DEVNET_RPC_URL: &str = "https://api.devnet.solana.com";
/// MagicBlock EU Devnet endpoint
pub const MAGICBLOCK_EU_DEVNET: &str = "https://devnet-eu.magicblock.app";

// Resource to hold Solana integration state
#[derive(Resource)]
pub struct SolanaIntegrationState {
    /// Pubkey provided by the Tauri Phantom/Solflare wallet
    pub wallet_pubkey: Option<Pubkey>,
    /// Local ephemeral session keypair (for ER session-key flows, NOT the main wallet)
    pub session_keypair: Option<Keypair>,
    /// Direct RPC client for Solana
    pub rpc_client: Option<RpcClient>,
    /// Current balance of the wallet
    pub balance: f64,
    /// Whether the handshake with opponent is completed
    pub handshake_completed: bool,
    /// Pending transaction task
    pub pending_task: Option<tokio::task::JoinHandle<Result<u64, String>>>,
    /// The opponent's public key (for verification)
    pub opponent_pubkey: Option<Pubkey>,
    /// Program ID for XFChess
    pub program_id: Pubkey,
}

impl std::fmt::Debug for SolanaIntegrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaIntegrationState")
            .field("session_keypair_pubkey", &self.session_keypair.as_ref().map(|k| k.pubkey()))
            .field("balance", &self.balance)
            .field("handshake_completed", &self.handshake_completed)
            .field("opponent_pubkey", &self.opponent_pubkey)
            .finish()
    }
}

impl Default for SolanaIntegrationState {
    fn default() -> Self {
        Self {
            wallet_pubkey: None,
            session_keypair: None,
            rpc_client: None,
            balance: 0.0,
            handshake_completed: false,
            pending_task: None,
            opponent_pubkey: None,
            program_id: SOLANA_PROGRAM_ID.parse().unwrap_or_default(),
        }
    }
}

impl SolanaIntegrationState {
    /// Derive a Program Derived Address (PDA) using the program ID
    pub fn derive_pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program_id).0
    }

    /// Get the game PDA for a given game ID
    pub fn get_game_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[GAME_SEED, &game_id.to_le_bytes()])
    }

    /// Get the escrow PDA for a given game ID
    pub fn get_escrow_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()])
    }

    /// Get the profile PDA for a given wallet
    pub fn get_profile_pda(&self, wallet: &Pubkey) -> Pubkey {
        self.derive_pda(&[PROFILE_SEED, wallet.as_ref()])
    }

    /// Get the move log PDA for a given game ID
    pub fn get_move_log_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[MOVE_LOG_SEED, &game_id.to_le_bytes()])
    }

    /// Get the session delegation PDA for a game and player
    pub fn get_session_delegation_pda(&self, game_id: u64, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ])
    }

    /// Create a new RPC client
    pub fn create_rpc_client(rpc_url: &str) -> RpcClient {
        RpcClient::new(rpc_url.to_string())
    }
}

/// Timer resource to rate-limit the devnet RPC balance poll.
#[derive(Resource)]
pub struct BalanceRefreshTimer(pub Timer);

impl Default for BalanceRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(5.0, TimerMode::Repeating))
    }
}
