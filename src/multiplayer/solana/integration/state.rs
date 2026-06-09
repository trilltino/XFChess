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
    /// Global persistent session keypair (loaded from disk, valid for 30 days).
    /// Used by `global_create_game` / `global_join_game` — no popup per game.
    pub global_session_keypair: Option<Keypair>,
    /// Whether the global session is active and loaded.
    pub global_session_active: bool,
    /// Direct RPC client for Solana
    pub rpc_client: Option<RpcClient>,
    /// Current balance of the wallet (SOL)
    pub balance: f64,
    /// Cached USD value of the wallet balance
    pub cached_usd_balance: Option<f64>,
    /// Latest SOL/USD exchange rate
    pub sol_usd_rate: Option<f64>,
    /// Whether the handshake with opponent is completed
    pub handshake_completed: bool,
    /// Pending transaction task
    pub pending_task: Option<tokio::task::JoinHandle<Result<u64, String>>>,
    /// The opponent's public key (for verification)
    pub opponent_pubkey: Option<Pubkey>,
    /// Program ID for XFChess
    pub program_id: Pubkey,
    /// Profile status for the connected wallet
    pub profile_status: ProfileStatus,
    /// Whether profile check is in progress
    pub checking_profile: bool,
    /// Pending async profile check task — returns (status, elo, display_name)
    pub pending_profile_check: Option<tokio::task::JoinHandle<Result<(ProfileStatus, Option<u16>, Option<String>), String>>>,
    /// Cached on-chain ELO (populated after profile lookup; 0 = unknown)
    pub cached_elo: u16,
    /// Cached display name from on-chain profile
    pub cached_display_name: Option<String>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum ProfileStatus {
    #[default]
    Unknown,
    NoProfile,
    HasProfileNoUsername,
    HasProfileWithUsername,
}

impl std::fmt::Debug for SolanaIntegrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaIntegrationState")
            .field("session_keypair_pubkey", &self.session_keypair.as_ref().map(|k| k.pubkey()))
            .field("global_session_active", &self.global_session_active)
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
            global_session_keypair: None,
            global_session_active: false,
            rpc_client: None,
            balance: 0.0,
            cached_usd_balance: None,
            sol_usd_rate: None,
            handshake_completed: false,
            pending_task: None,
            opponent_pubkey: None,
            program_id: SOLANA_PROGRAM_ID.parse().unwrap_or_default(),
            profile_status: ProfileStatus::Unknown,
            checking_profile: false,
            pending_profile_check: None,
            cached_elo: 0,
            cached_display_name: None,
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

    /// Get the global session delegation PDA for `player`.
    pub fn get_global_session_pda(&self, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[b"global_session", player.as_ref()])
    }

    /// Try to load the global session keypair from disk for `wallet`.
    /// Sets `global_session_keypair` and `global_session_active` accordingly.
    pub fn try_load_global_session(&mut self, wallet: &Pubkey) {
        use crate::multiplayer::solana::global_session_manager::GlobalSessionKeyManager;
        match GlobalSessionKeyManager::load(wallet) {
            Ok(mgr) => {
                let arc_kp = mgr.signer();
                if let Ok(kp) = Keypair::from_bytes(&arc_kp.to_bytes()) {
                    self.global_session_keypair = Some(kp);
                    self.global_session_active = true;
                }
            }
            Err(_) => {
                self.global_session_keypair = None;
                self.global_session_active = false;
            }
        }
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
        // 60 s: on-chain ELO only changes after finalize_game (minutes apart)
        Self(Timer::from_seconds(60.0, TimerMode::Repeating))
    }
}
