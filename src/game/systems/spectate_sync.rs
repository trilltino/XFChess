use crate::core::states::GameMode;
use crate::engine::board_state::ChessEngine;
use crate::game::events::NetworkMoveEvent;
use crate::game::resources::MoveHistory;
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::{MessageReader, MessageWriter};
use crate::multiplayer::types::NetworkEvent;
use bevy::prelude::*;

/// Resource to track if we've successfully synced the initial board state
#[derive(Resource, Default)]
pub struct SpectatorSyncStatus {
    pub initialized: bool,
    pub last_move_uci: Option<String>,
}

/// Plugin to handle high-fidelity board synchronization for spectators
pub struct SpectateSyncPlugin;

impl Plugin for SpectateSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSyncStatus>()
            .add_systems(Update, sync_spectator_board.run_if(is_spectator));
    }
}

/// Run condition check for spectator mode
pub fn is_spectator(game_mode: Res<GameMode>) -> bool {
    *game_mode == GameMode::Spectator
}

/// Main system that listens for spectator broadcasts and updates the visual board
pub fn sync_spectator_board(
    mut network_events: MessageReader<NetworkEvent>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    mut engine: ResMut<ChessEngine>,
    mut sync_status: ResMut<SpectatorSyncStatus>,
    mut _move_history: ResMut<MoveHistory>,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::GameStateBroadcast {
            game_id: _,
            fen,
            last_move,
            move_number: _,
            is_check: _,
        }) = event
        {
            // 1. Handle Initial Sync (Hard Teleport)
            if !sync_status.initialized {
                info!("[SPECTATE] Initial sync from FEN: {}", fen);
                if let Err(e) = engine.set_from_fen(&fen) {
                    error!("[SPECTATE] Failed to sync board from FEN: {}", e);
                }
                sync_status.initialized = true;
                sync_status.last_move_uci = last_move.clone();
                continue;
            }

            // 2. Handle Subsequent Moves (Animation)
            if let Some(uci) = last_move {
                if Some(uci.clone()) != sync_status.last_move_uci {
                    info!("[SPECTATE] Detected new move: {}", uci);

                    if uci.len() >= 4 {
                        let from_file =
                            (uci.as_bytes()[0] as char).to_digit(36).unwrap_or(0) as u8 - 10;
                        let from_rank =
                            (uci.as_bytes()[1] as char).to_digit(10).unwrap_or(0) as u8 - 1;
                        let to_file =
                            (uci.as_bytes()[2] as char).to_digit(36).unwrap_or(0) as u8 - 10;
                        let to_rank =
                            (uci.as_bytes()[3] as char).to_digit(10).unwrap_or(0) as u8 - 1;
                        let promotion = uci.get(4..5).map(|s| s.chars().next().unwrap());

                        move_events.write(NetworkMoveEvent {
                            from: (from_file, from_rank),
                            to: (to_file, to_rank),
                            promotion,
                            expected_fen: None,
                        });

                        sync_status.last_move_uci = Some(uci.clone());
                    }
                }
            }
        }
    }
}
