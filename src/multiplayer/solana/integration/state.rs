use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

use crate::solana::instructions::{
    GAME_SEED, MOVE_LOG_SEED, PROFILE_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
    SESSION_DELEGATION_SEED, WAGER_ESCROW_SEED,
};

pub static DEVNET_RPC_URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| format!("{}/api/rpc", crate::multiplayer::network::vps::vps_base()))
});
pub const MAGICBLOCK_EU_DEVNET: &str = "https://devnet-eu.magicblock.app";

// Resource to hold Solana integration state
#[derive(Resource)]
pub struct SolanaIntegrationState {
    pub wallet_pubkey: Option<Pubkey>,
    pub session_keypair: Option<Keypair>,
    pub global_session_keypair: Option<Keypair>,
    pub global_session_active: bool,
    pub global_session_setup_in_progress: bool,
    pub wallet_is_embedded: bool,
    pub global_session_unavailable_reason: Option<String>,
    pub rpc_client: Option<RpcClient>,
    pub balance: f64,
    pub cached_usd_balance: Option<f64>,
    pub sol_usd_rate: Option<f64>,
    pub handshake_completed: bool,
    pub pending_task: Option<tokio::task::JoinHandle<Result<u64, String>>>,
    pub opponent_pubkey: Option<Pubkey>,
    pub program_id: Pubkey,
    pub profile_status: ProfileStatus,
    pub checking_profile: bool,
    pub pending_profile_check: Option<
        tokio::task::JoinHandle<Result<(ProfileStatus, Option<u16>, Option<String>), String>>,
    >,
    pub cached_elo: u16,
    pub cached_display_name: Option<String>,
    pub pending_participants_fetch: Option<(
        u64,
        tokio::task::JoinHandle<Result<Option<(String, String)>, String>>,
    )>,
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
            .field(
                "session_keypair_pubkey",
                &self.session_keypair.as_ref().map(|k| k.pubkey()),
            )
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
            global_session_setup_in_progress: false,
            // Conservative default: unknown wallets take the per-game signing
            // path until the bridge confirms an embedded wallet.
            wallet_is_embedded: false,
            global_session_unavailable_reason: None,
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
            pending_participants_fetch: None,
        }
    }
}

impl SolanaIntegrationState {
    pub fn derive_pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program_id).0
    }

    pub fn get_game_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[GAME_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_escrow_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_profile_pda(&self, wallet: &Pubkey) -> Pubkey {
        self.derive_pda(&[PROFILE_SEED, wallet.as_ref()])
    }

    pub fn get_move_log_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[MOVE_LOG_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_session_delegation_pda(&self, game_id: u64, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ])
    }

    pub fn get_global_session_pda(&self, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[b"global_session", player.as_ref()])
    }

    pub fn try_load_global_session(&mut self, wallet: &Pubkey) {
        use crate::multiplayer::solana::global_session_manager::GlobalSessionKeyManager;
        match GlobalSessionKeyManager::load(wallet) {
            Ok(mgr) => {
                let arc_kp = mgr.signer();
                if let Ok(kp) = Keypair::try_from(arc_kp.to_bytes().as_slice()) {
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

    pub fn create_rpc_client(rpc_url: &str) -> RpcClient {
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed())
    }
}

#[derive(Resource)]
pub struct BalanceRefreshTimer(pub Timer);

impl Default for BalanceRefreshTimer {
    fn default() -> Self {
        // 15s: this balance feeds lobby.cached_balance, which gates whether
        // a wager can be joined/created (see sync_from_solana_state) — it
        // needs to track real changes (e.g. a wager just settling) during an
        // active session, not just the initial connect (which now fetches
        // immediately in update_wallet_balance regardless of this timer).
        Self(Timer::from_seconds(15.0, TimerMode::Repeating))
    }
}
