#![cfg(feature = "solana")]
use bevy::prelude::*;

use crate::multiplayer::{
    network_protocol::NetworkMessage,
    rollup_manager::{EphemeralRollupManager, EphemeralRollupPlugin},
    rollup_network_bridge::RollupNetworkBridgePlugin,
    session_key_manager::SessionKeyManager,
    BraidNetworkState,
};
use crate::multiplayer::SolanaWallet;

#[derive(Resource, Default)]
pub struct EphemeralMvpState {
    pub is_initialized: bool,
    pub game_finalized: bool,
}

impl EphemeralMvpState {
    pub fn start_game(&mut self, _game_id: u64, _initial_fen: String) {
        self.is_initialized = true;
        self.game_finalized = false;
    }

    pub fn finalize_game(&mut self) {
        self.game_finalized = true;
    }

    pub fn is_game_active(&self) -> bool {
        self.is_initialized && !self.game_finalized
    }
}

pub struct EphemeralMvpPlugin;

impl Plugin for EphemeralMvpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EphemeralMvpState>()
            .init_resource::<SessionKeyManager>()
            .add_plugins(EphemeralRollupPlugin)
            .add_plugins(RollupNetworkBridgePlugin)
            .add_systems(Update, (broadcast_session_info, handle_game_finalization));
    }
}

fn broadcast_session_info(
    mvp_state: Res<EphemeralMvpState>,
    rollup_manager: Res<EphemeralRollupManager>,
    session_key_manager: Res<SessionKeyManager>,
    network_state: Res<BraidNetworkState>,
    #[cfg(feature = "solana")] wallet: Option<Res<SolanaWallet>>,
) {
    if !mvp_state.is_initialized {
        return;
    }
    #[cfg(feature = "solana")]
    {
        let Some(wallet_ref) = wallet else { return };
        let Some(session_pubkey) = session_key_manager.get_session_pubkey() else {
            return;
        };

        let expires_at = chrono::Utc::now().timestamp() + (2 * 60 * 60);

        if let Some(tx) = &network_state.message_sender {
            if let Some(player_pubkey) = wallet_ref.pubkey {
                let _ = tx.send(NetworkMessage::SessionInfo {
                    game_id: rollup_manager.game_id,
                    player_pubkey,
                    session_pubkey,
                    expires_at,
                });
            }
        }
    }
}

fn handle_game_finalization(
    mut mvp_state: ResMut<EphemeralMvpState>,
    mut rollup_manager: ResMut<EphemeralRollupManager>,
) {
    if !mvp_state.game_finalized {
        return;
    }
    if let Some((moves, _)) = rollup_manager.force_flush() {
        info!(
            "Force-flushed {} moves before finalization for game {}",
            moves.len(),
            rollup_manager.game_id
        );
    }
    mvp_state.game_finalized = false;
    rollup_manager.reset();
}
