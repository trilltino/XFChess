//! Wagered on-chain lobbies: create ([`create`]), join ([`join`]) and cancel
//! ([`cancel`]) games, in-game draw and timeout claims ([`game_actions`]), and the
//! per-frame task polling ([`polling`]).

use bevy::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::multiplayer::solana::integration::state::DEVNET_RPC_URL;
use crate::solana::instructions::{
    accept_draw_ix, authorize_session_key_ix, claim_timeout_ix, create_game_ix, join_game_ix,
    offer_draw_ix, GAME_SEED, PROGRAM_ID as SOLANA_PROGRAM_ID,
};

mod cancel;
mod create;
mod game_actions;
mod join;
mod polling;
pub use cancel::*;
pub use create::*;
pub use game_actions::*;
pub use join::*;
pub use polling::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyMode {
    #[default]
    Create,
    Join,
    Browse,
    Tournament,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EloMatchPref {
    Strict, // ±50 ELO
    #[default]
    Expanded, // ±150 ELO
    Any,    // no filter
}

impl EloMatchPref {
    pub fn label(self) -> &'static str {
        match self {
            Self::Strict => "Strict ±50",
            Self::Expanded => "Normal ±150",
            Self::Any => "Any ELO",
        }
    }

    pub fn range(self) -> Option<u16> {
        match self {
            Self::Strict => Some(50),
            Self::Expanded => Some(150),
            Self::Any => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum LobbyStatus {
    #[default]
    Idle,
    Pending,
    Success(u64),
    Fetched {
        wager_sol: f64,
        game_id: u64,
    },
    WaitingForOpponent {
        game_id: u64,
    },
    OpponentJoined {
        game_id: u64,
    },
    WaitingForHostStart {
        game_id: u64,
    },
    EnterGame {
        game_id: u64,
    },
    Cancelling {
        game_id: u64,
    },
    Cancelled {
        game_id: u64,
        refunded: bool,
        message: String,
    },
    CancelFailed {
        game_id: u64,
        error: String,
    },
    Error(String),
}

#[derive(Resource)]
pub struct SolanaLobbyState {
    pub mode: LobbyMode,
    pub allow_create: bool,
    pub wager_sol: f32,
    pub wager_amount_input: String,
    pub match_type: u8,
    pub game_id_input: String,
    pub status: LobbyStatus,
    pub tx_rx: Option<oneshot::Receiver<Result<u64, String>>>,
    pub lookup_rx: Option<oneshot::Receiver<Result<(u64, u64), String>>>,
    pub opponent_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    pub game_start_poll_rx: Option<oneshot::Receiver<Result<(), String>>>,
    pub cancel_rx: Option<oneshot::Receiver<Result<CancelOutcome, String>>>,
    pub money_flow: crate::multiplayer::solana::money_flow::MoneyFlowState,
    // Cached from SolanaIntegrationState each frame.
    pub cached_balance: f64,
    pub cached_keypair_bytes: Option<Vec<u8>>,
    pub cached_rpc_url: String,
    pub cached_global_session_keypair_bytes: Option<Vec<u8>>,
    pub last_attempt_used_global_session: bool,
    pub cached_display_name: Option<String>,
    pub cached_node_id: Option<String>,
    pub cached_secret_key_bytes: Option<[u8; 32]>,
    pub cached_elo: u16,
    pub cached_region: Option<String>,
    pub room_password: Option<String>,
    pub time_control_base: u32,
    pub time_control_inc: u32,
    pub elo_pref: EloMatchPref,
    pub rejoin_rx: Option<oneshot::Receiver<Result<Option<u64>, String>>>,
    pub rejoin_game_id: Option<u64>,
    pub rejoin_lookup_error: Option<String>,
    /// Chain-backed scan for stakes this wallet can still recover — see
    /// `wager_recovery`. Re-run on wallet change and after a cancellation.
    pub recovery_rx:
        Option<oneshot::Receiver<crate::multiplayer::solana::wager_recovery::WagerScan>>,
    pub recovery_scan: Option<crate::multiplayer::solana::wager_recovery::WagerScan>,
    pub recovery_action_rx: Option<oneshot::Receiver<Result<String, String>>>,
    pub recovery_message: Option<Result<String, String>>,
    pub recovery_refresh_at: Option<std::time::Instant>,
    pub browse_games: Vec<crate::multiplayer::network::p2p_vps::VpsGameListing>,
    browse_revision: Option<u64>,
    pub tournament_games: Vec<crate::multiplayer::network::vps::TournamentGameListing>,
    pub tournament_last_fetch: Option<std::time::Instant>,
    pub tournament_rx: Option<
        crossbeam_channel::Receiver<Vec<crate::multiplayer::network::vps::TournamentGameListing>>,
    >,
    pub announce_warning: Option<String>,
    pub last_lobby_heartbeat: Option<std::time::Instant>,
}

impl Default for SolanaLobbyState {
    fn default() -> Self {
        Self {
            mode: LobbyMode::default(),
            allow_create: true,
            wager_sol: 0.0,
            wager_amount_input: String::new(),
            match_type: 0,
            game_id_input: String::new(),
            status: LobbyStatus::default(),
            tx_rx: None,
            lookup_rx: None,
            opponent_poll_rx: None,
            game_start_poll_rx: None,
            cancel_rx: None,
            money_flow: crate::multiplayer::solana::money_flow::MoneyFlowState::default(),
            cached_balance: 0.0,
            cached_keypair_bytes: None,
            cached_rpc_url: DEVNET_RPC_URL.to_string(),
            cached_global_session_keypair_bytes: None,
            last_attempt_used_global_session: false,
            cached_display_name: None,
            cached_node_id: None,
            cached_secret_key_bytes: None,
            cached_elo: 0,
            cached_region: None,
            room_password: None,
            time_control_base: 300,
            time_control_inc: 0,
            elo_pref: EloMatchPref::default(),
            rejoin_rx: None,
            rejoin_game_id: None,
            rejoin_lookup_error: None,
            recovery_rx: None,
            recovery_scan: None,
            recovery_action_rx: None,
            recovery_message: None,
            recovery_refresh_at: None,
            browse_games: Vec::new(),
            browse_revision: None,
            tournament_games: Vec::new(),
            tournament_last_fetch: None,
            tournament_rx: None,
            announce_warning: None,
            last_lobby_heartbeat: None,
        }
    }
}

impl SolanaLobbyState {
    pub fn wager_lamports(&self) -> u64 {
        (self.wager_sol as f64 * 1_000_000_000.0) as u64
    }

    pub fn start_recovery_scan(&mut self, wallet: Pubkey) {
        let (tx, rx) = oneshot::channel();
        crate::multiplayer::solana::wager_recovery::spawn_scan(
            wallet,
            self.cached_rpc_url.clone(),
            tx,
        );
        self.recovery_rx = Some(rx);
    }

    /// Refresh the recovery list from chain after a cancellation resolves, so
    /// it reflects the confirmed escrow state rather than the HTTP/UI result.
    pub fn rescan_recovery(&mut self) {
        let wallet = self
            .cached_keypair_bytes
            .as_deref()
            .and_then(|b| Pubkey::try_from(b).ok());
        if let Some(wallet) = wallet {
            self.start_recovery_scan(wallet);
        }
    }
}

pub struct SolanaLobbyPlugin;

impl Plugin for SolanaLobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolanaLobbyState>()
            .init_resource::<crate::multiplayer::solana::addon::SolanaGameSync>()
            .init_resource::<crate::multiplayer::solana::addon::CompetitiveMatchState>()
            .add_systems(
                Update,
                (
                    sync_from_solana_state,
                    poll_lobby_tasks,
                    poll_rejoin_check,
                    poll_recovery_scan,
                    poll_solana_browse,
                    poll_tournament_games,
                )
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Lobby),
            );
    }
}
