use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

use crate::solana::instructions::{
    GAME_SEED, MOVE_LOG_SEED, PROFILE_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
    SESSION_DELEGATION_SEED, WAGER_ESCROW_SEED,
};

/// Devnet RPC endpoint. Overridable via `SOLANA_RPC_URL` (same var name and
/// resolution as the backend's `SigningConfig` and `vps_base()`) so a local
/// dev build can point directly at a dedicated provider (e.g. Triton).
///
/// Distributed builds never carry a real provider URL/token here — this
/// constant ships in every player's binary, so a literal secret would leak to
/// all of them. Instead the default resolves to the backend's `/api/rpc`
/// proxy (`routes::rpc_proxy`), which forwards allow-listed JSON-RPC calls to
/// the real (paid, non-rate-limited) endpoint server-side, where the token
/// stays — same base host as every other VPS call, see `vps_base()`.
pub static DEVNET_RPC_URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| format!("{}/api/rpc", crate::multiplayer::network::vps::vps_base()))
});
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
    /// True while `authorize_global_session_if_needed` has a background
    /// authorization attempt in flight. Lets the lobby UI show a "setting up
    /// one-time signing..." indicator (and hold off on Create/Join) instead
    /// of silently racing into the per-game wallet-popup fallback path just
    /// because the one-time setup hasn't resolved yet this frame.
    pub global_session_setup_in_progress: bool,
    /// True when the connected wallet is a Privy embedded wallet rather than a
    /// browser extension. Synced from the Tauri bridge's `/status` by
    /// `sync_bridge_pubkey_to_solana` (main_menu.rs).
    ///
    /// Gates `authorize_global_session_if_needed`: the no-popup flow runs only
    /// for embedded wallets, because its one unresolved blocker — a Solflare
    /// "network mismatch: current network devnet, but this transaction is for
    /// mainnet" rejection arriving with no user action — is an artifact of an
    /// extension carrying its own user-selected cluster, which an embedded
    /// wallet does not have. Defaults to `false`, so anything unknown takes the
    /// proven per-game signing path.
    pub wallet_is_embedded: bool,
    /// Set once `authorize_global_session_if_needed` permanently gives up on
    /// this wallet for the run (see `MAX_ATTEMPTS`). `Some(reason)` means
    /// every game this session will use per-game wallet signing instead of
    /// the zero-popup path — the lobby UI surfaces this explicitly instead of
    /// letting the fallback (and its bundled platform-fee popup) show up
    /// unexplained.
    pub global_session_unavailable_reason: Option<String>,
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
    pub pending_profile_check: Option<
        tokio::task::JoinHandle<Result<(ProfileStatus, Option<u16>, Option<String>), String>>,
    >,
    /// Cached on-chain ELO (populated after profile lookup; 0 = unknown)
    pub cached_elo: u16,
    /// Cached display name from on-chain profile
    pub cached_display_name: Option<String>,
    /// Pending async fetch of `CausalChainState::verified_wallets`'s source
    /// data for the game named by the `u64` — spawned once per game start by
    /// `spawn_verified_participants_fetch`, consumed by
    /// `poll_verified_participants_fetch`. See
    /// `docs/plans/networking-hardening-plan.md`'s Phase C.
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

    /// Create a new RPC client at `confirmed` commitment — the client default
    /// (`finalized`) waits for ~32 confirmations (~13-20s) where ~1-2 (~1-2s)
    /// is already safe for interactive use.
    pub fn create_rpc_client(rpc_url: &str) -> RpcClient {
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed())
    }
}

/// Timer resource to rate-limit the devnet RPC balance poll.
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
