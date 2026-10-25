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
    pub last_fen: Option<String>,
}

pub struct SpectateSyncPlugin;

impl Plugin for SpectateSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSyncStatus>()
            .add_systems(
                Update,
                sync_spectator_board
                    .run_if(is_spectator)
                    .in_set(crate::core::schedule::MoveSource::Spectator),
            )
            .add_systems(
                OnEnter(crate::core::GameState::InGame),
                |mut status: ResMut<SpectatorSyncStatus>| *status = SpectatorSyncStatus::default(),
            );
    }
}

pub fn is_spectator(game_mode: Res<GameMode>) -> bool {
    *game_mode == GameMode::Spectator
}

pub fn sync_spectator_board(
    mut network_events: MessageReader<NetworkEvent>,
    mut move_events: MessageWriter<NetworkMoveEvent>,
    mut snapshot: ResMut<super::board_snapshot::PendingBoardSnapshot>,
    mut sync_status: ResMut<SpectatorSyncStatus>,
    mut _move_history: ResMut<MoveHistory>,
    session: Option<Res<crate::multiplayer::spectator::SpectatorSession>>,
) {
    let mut sequential = Vec::new();
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::GameStateBroadcast {
            game_id,
            fen,
            last_move,
            move_number: _,
            is_check: _,
        }) = event
        {
            if let Some(session) = &session {
                // Broadcasts are a live feed. Delayed spectators keep using the
                // existing delayed move dispatcher, and old-game traffic must
                // never replace the current board.
                if session.delayed
                    || !session.delay_checked
                    || session
                        .game_id
                        .as_deref()
                        .map(crate::multiplayer::network::online_game_session::numeric_game_id)
                        != Some(*game_id)
                {
                    continue;
                }
            }
            if sync_status.last_fen.as_ref() == Some(fen) {
                continue;
            }
            if xfchess_rules::Game::from_fen(fen).is_err() {
                warn!("[SPECTATE] Ignoring invalid snapshot");
                continue;
            }
            // Animate only a verified single step from the last broadcast.
            // A jump to live position or a missed broadcast replaces the board.
            let step = sync_status
                .last_fen
                .as_ref()
                .zip(last_move.as_ref())
                .and_then(|(previous, uci)| {
                    let mut game = xfchess_rules::Game::from_fen(previous).ok()?;
                    game.play_uci(uci).ok()?;
                    (game.fen() == *fen)
                        .then(|| ChessEngine::parse_uci_move(uci))
                        .flatten()
                });
            if let Some((from, to, promotion)) = step {
                sequential.push(NetworkMoveEvent {
                    from,
                    to,
                    promotion,
                    expected_fen: Some(fen.clone()),
                    dedup_version: None,
                });
            } else {
                sequential.clear();
                snapshot.0 = Some(fen.clone());
            }
            sync_status.initialized = true;
            sync_status.last_move_uci = last_move.clone();
            sync_status.last_fen = Some(fen.clone());
        }
    }
    move_events.write_batch(sequential);
}
