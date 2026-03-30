//! Tournament client state, Bevy resources, and on-chain instruction dispatch
//! for the 4-player bracket system.

use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;

use crate::multiplayer::rollup::vps_client::{MatchAssignmentResp, TournamentSummary};

// ── Join status ────────────────────────────────────────────────────────────────

/// Tracks the state of an in-flight or completed tournament registration.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum TournamentJoinStatus {
    #[default]
    Idle,
    /// On-chain + VPS registration transaction in flight.
    Pending,
    /// Registration succeeded — stores the assigned slot (0–3).
    Registered(usize),
    Error(String),
}

// ── Resources ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TournamentClientState {
    /// Currently browsed / joined tournament ID.
    pub active_tournament_id: Option<u64>,
    /// Player slot within the tournament (0–3).
    pub my_slot: Option<usize>,
    /// Cached list of available tournaments from VPS.
    pub available_tournaments: Vec<TournamentSummary>,
    /// Current match assignment (set once bracket is active).
    pub my_match: Option<MatchAssignmentResp>,
    /// Status text shown in lobby / waiting screens.
    pub status_message: String,
    /// Polling timer: seconds since last my-match poll.
    pub poll_timer: f32,
    /// Registration state machine.
    pub join_status: TournamentJoinStatus,
    /// Oneshot receiver for an in-flight `register_player` transaction.
    pub tx_rx: Option<oneshot::Receiver<Result<usize, String>>>,
}

impl Default for TournamentClientState {
    fn default() -> Self {
        Self {
            active_tournament_id: None,
            my_slot: None,
            available_tournaments: Vec::new(),
            my_match: None,
            status_message: String::new(),
            poll_timer: 0.0,
            join_status: TournamentJoinStatus::Idle,
            tx_rx: None,
        }
    }
}

impl TournamentClientState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_registered(&self) -> bool {
        matches!(self.join_status, TournamentJoinStatus::Registered(_))
    }

    pub fn has_match_assigned(&self) -> bool {
        self.my_match
            .as_ref()
            .map(|m| m.found && m.opponent_pubkey.is_some())
            .unwrap_or(false)
    }
}

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Message, Debug, Clone)]
pub struct OpenTournamentDiscoveryEvent;

#[derive(Message, Debug, Clone)]
pub struct RegisterForTournamentEvent {
    pub tournament_id: u64,
}

#[derive(Message, Debug, Clone)]
pub struct TournamentMatchAssignedEvent {
    pub tournament_id: u64,
    pub match_index: u8,
    pub game_id: Option<u64>,
    pub opponent_pubkey: String,
    pub opponent_node_id: Option<String>,
    pub your_color: String,
}

// ── On-chain instruction dispatch ─────────────────────────────────────────────

/// Spawn a background task on `IoTaskPool` that:
///   1. Builds the `register_player` on-chain instruction.
///   2. Sends it via the Tauri signing bridge → Phantom popup → confirmed on devnet.
///   3. Notifies the VPS backend via `POST /tournament/{id}/join`.
///   4. Returns the assigned slot index (or an error) through `tx`.
pub fn spawn_register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    elo: u32,
    tx: oneshot::Sender<Result<usize, String>>,
) {
    let program_id: Pubkey = crate::solana::instructions::PROGRAM_ID
        .parse()
        .unwrap_or_default();
    let rpc_url = crate::multiplayer::solana::integration::DEVNET_RPC_URL.to_string();

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result =
                async_register_tournament(rpc_url, program_id, tournament_id, wallet_pubkey, elo)
                    .await;
            let _ = tx.send(result);
        })
        .detach();
}

async fn async_register_tournament(
    rpc_url: String,
    program_id: Pubkey,
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    elo: u32,
) -> Result<usize, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::solana::instructions::{init_profile_ix, register_player_ix, PROFILE_SEED};
    use solana_client::rpc_client::RpcClient;

    // Check whether a PlayerProfile PDA already exists for this wallet.
    // If not, prepend init_profile so it gets created in the same transaction.
    let rpc = RpcClient::new(rpc_url.clone());
    let profile_pda = Pubkey::find_program_address(
        &[PROFILE_SEED, wallet_pubkey.as_ref()],
        &program_id,
    ).0;
    let needs_profile = rpc.get_account(&profile_pda).is_err();

    let register_ix = register_player_ix(program_id, wallet_pubkey, tournament_id)
        .map_err(|e| format!("build register_player_ix: {e}"))?;

    let mut ixs = Vec::new();
    if needs_profile {
        if let Ok(ix) = init_profile_ix(program_id, wallet_pubkey) {
            ixs.push(ix);
        }
    }
    ixs.push(register_ix);

    sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &ixs, &[])
        .map_err(|e| format!("wallet sign: {e}"))?;

    let slot = crate::multiplayer::rollup::vps_client::join_tournament(
        tournament_id,
        &wallet_pubkey.to_string(),
        elo,
    )?;

    Ok(slot)
}

// ── Bevy system ───────────────────────────────────────────────────────────────

/// Runs every frame in `Update`. Checks if the in-flight registration oneshot
/// has resolved and updates `TournamentClientState` + navigates to the lobby.
fn poll_tournament_tasks(
    mut tournament: ResMut<TournamentClientState>,
    mut menu_state: ResMut<NextState<crate::core::states::MenuState>>,
) {
    if let Some(ref mut rx) = tournament.tx_rx {
        match rx.try_recv() {
            Ok(Ok(slot)) => {
                tournament.my_slot = Some(slot);
                tournament.join_status = TournamentJoinStatus::Registered(slot);
                tournament.status_message =
                    format!("Registered in slot {}. Waiting for bracket…", slot + 1);
                tournament.tx_rx = None;
                menu_state.set(crate::core::states::MenuState::TournamentLobby);
                info!("[TOURNAMENT] Registration confirmed — slot {}", slot);
            }
            Ok(Err(e)) => {
                tournament.join_status = TournamentJoinStatus::Error(e.clone());
                tournament.status_message = format!("Registration failed: {}", e);
                tournament.tx_rx = None;
                warn!("[TOURNAMENT] Registration error: {}", e);
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(_) => {
                tournament.join_status =
                    TournamentJoinStatus::Error("Task dropped unexpectedly".to_string());
                tournament.status_message = "Registration task failed. Please retry.".to_string();
                tournament.tx_rx = None;
            }
        }
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct TournamentClientPlugin;

impl Plugin for TournamentClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TournamentClientState>()
            .add_systems(Update, poll_tournament_tasks);
    }
}
