use crate::core::state::GameMode;
use crate::engine::board_state::ChessEngine;
use crate::game::events::NetworkMoveEvent;
use crate::game::resources::MoveHistory;
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::traits::{MessageReader, MessageWriter};
use crate::multiplayer::types::NetworkEvent;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SpectatorSyncStatus {
    pub initialized: bool,
    pub last_move_uci: Option<String>,
}

pub struct SpectateSyncPlugin;

impl Plugin for SpectateSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSyncStatus>().add_systems(
            Update,
            sync_spectator_board
                .run_if(is_spectator)
                .in_set(crate::core::schedule::MoveSource::Spectator),
        );
    }
}

pub fn is_spectator(game_mode: Res<GameMode>) -> bool {
    *game_mode == GameMode::Spectator
}

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
            if !sync_status.initialized {
                info!("[SPECTATE] Initial sync from FEN: {}", fen);
                if let Err(e) = engine.set_from_fen(fen) {
                    error!("[SPECTATE] Failed to sync board from FEN: {}", e);
                }
                sync_status.initialized = true;
                sync_status.last_move_uci = last_move.clone();
                continue;
            }

            if let Some(uci) = last_move {
                if sync_status.last_move_uci.as_ref() != Some(uci) {
                    info!("[SPECTATE] Detected new move: {}", uci);

                    // The move comes off the network: parse it, never trust it.
                    if let Some((from, to, promotion)) = ChessEngine::parse_uci_move(uci) {
                        move_events.write(NetworkMoveEvent {
                            from,
                            to,
                            promotion,
                            expected_fen: None,
                            dedup_version: None,
                        });

                        sync_status.last_move_uci = Some(uci.clone());
                    } else {
                        warn!("[SPECTATE] Ignoring malformed move {:?}", uci);
                    }
                }
            }
        }
    }
}
