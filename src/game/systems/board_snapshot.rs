//! Full-position replacement, used by trusted resync and spectator snapshots.
//! Snapshot reconstruction is authority work; it never publishes a chess move.

use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, Piece};
use crate::game::resources::{CurrentTurn, PendingPromotion, PendingTurnAdvance, Selection};
use crate::motion::{board_to_world, square_to_board, BoardEpoch};
use crate::rendering::pieces::{
    spawn_pieces_from_fen, PieceMeshes, PieceSpriteHandles, PiecesSpawned,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PendingBoardSnapshot(pub Option<String>);

/// A rules-verified checkpoint, scoped to the current game and board. Peer
/// responses may repair a conflicting local FEN only by matching this checkpoint.
#[derive(Resource, Debug, Clone)]
pub struct VerifiedBoardPosition {
    pub game_id: u64,
    pub epoch: u64,
    pub fen: String,
}

impl VerifiedBoardPosition {
    pub fn record(world: &mut World, fen: String) {
        let game_id = world
            .get_resource::<crate::multiplayer::network::online_game_session::OnlineGameSession>()
            .filter(|session| session.active)
            .map(|session| {
                crate::multiplayer::network::online_game_session::numeric_game_id(&session.game_id)
            })
            .unwrap_or_default();
        let epoch = world
            .get_resource::<BoardEpoch>()
            .copied()
            .unwrap_or_default()
            .0;
        world.insert_resource(Self {
            game_id,
            epoch,
            fen,
        });
    }

    pub fn matches(&self, game_id: u64, epoch: u64, fen: &str) -> bool {
        self.game_id == game_id && self.epoch == epoch && self.fen == fen
    }
}

/// Routes already verified recovery positions to the active board. A rollup
/// checkpoint must not rewind optimistic local moves or bypass spectator delay.
#[derive(SystemParam)]
pub struct RecoveryBoard<'w> {
    state: Option<Res<'w, State<crate::core::GameState>>>,
    engine: Option<Res<'w, ChessEngine>>,
    snapshot: Option<ResMut<'w, PendingBoardSnapshot>>,
    mode: Option<Res<'w, crate::core::GameMode>>,
    session: Option<Res<'w, crate::multiplayer::network::online_game_session::OnlineGameSession>>,
    spectator: Option<Res<'w, crate::multiplayer::spectator::SpectatorSession>>,
    checkpoint: Option<Res<'w, VerifiedBoardPosition>>,
    epoch: Option<Res<'w, BoardEpoch>>,
}

impl RecoveryBoard<'_> {
    pub fn is_spectator(&self) -> bool {
        self.mode
            .as_ref()
            .is_some_and(|mode| **mode == crate::core::GameMode::Spectator)
    }
    pub fn has_checkpoint(&self, game_id: u64, fen: &str) -> bool {
        self.checkpoint.as_ref().is_some_and(|checkpoint| {
            self.epoch
                .as_ref()
                .is_some_and(|epoch| checkpoint.matches(game_id, epoch.0, fen))
        })
    }

    pub fn queue(&mut self, game_id: u64, fen: &str) {
        if self
            .state
            .as_ref()
            .is_some_and(|state| *state.get() != crate::core::GameState::InGame)
        {
            return;
        }
        let permitted =
            self.mode.as_ref().is_some_and(|mode| {
                if **mode == crate::core::GameMode::Spectator {
                    self.spectator.as_ref().is_some_and(|session| {
                        session.delay_checked
                            && !session.delayed
                            && session.game_id.as_deref().map(
                                crate::multiplayer::network::online_game_session::numeric_game_id,
                            ) == Some(game_id)
                    })
                } else {
                    crate::multiplayer::types::is_online_game_mode(**mode)
                    && self.session.as_ref().is_some_and(|session| {
                        session.active
                            && crate::multiplayer::network::online_game_session::numeric_game_id(
                                &session.game_id,
                            ) == game_id
                    })
                }
            });
        if !permitted {
            return;
        }
        let Some(engine) = &self.engine else { return };
        let (Some(incoming), Some(current)) = (
            crate::multiplayer::systems::fen_ply(fen),
            crate::multiplayer::systems::fen_ply(engine.current_fen()),
        ) else {
            return;
        };
        if incoming < current {
            return;
        }
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.0 = Some(fen.to_string());
        }
    }
}

#[derive(SystemParam)]
pub struct SnapshotStatus<'w> {
    game_over: Option<ResMut<'w, crate::game::resources::GameOverState>>,
    prompt: Option<ResMut<'w, crate::game::resources::PendingGameOver>>,
    turn_context: Option<ResMut<'w, crate::game::resources::TurnStateContext>>,
}

pub fn apply_board_snapshot(
    mut snapshot: ResMut<PendingBoardSnapshot>,
    mut engine: ResMut<ChessEngine>,
    mut turn: ResMut<CurrentTurn>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut promotion: ResMut<PendingPromotion>,
    mut selection: ResMut<Selection>,
    mut epoch: ResMut<BoardEpoch>,
    mut premove: ResMut<crate::game::resources::premove::Premove>,
    mut commands: Commands,
    pieces: Query<Entity, With<Piece>>,
    meshes: Option<Res<PieceMeshes>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    sprites: Option<Res<PieceSpriteHandles>>,
    spawned: Option<ResMut<PiecesSpawned>>,
    status: SnapshotStatus,
) {
    let Some(fen) = snapshot.0.take() else { return };
    if let Err(error) = engine.set_from_fen(&fen) {
        warn!("[BOARD] Ignoring invalid snapshot: {error}");
        return;
    }
    engine.rebuild_legal_move_cache();
    engine.synced_this_move = true;
    turn.color = engine.current_turn;
    turn.move_number = engine.fullmove_counter;
    *pending_turn = PendingTurnAdvance::default();
    *selection = Selection::default();
    promotion.clear();
    if let Some(mut game_over) = status.game_over {
        *game_over = Default::default();
    }
    if let Some(mut prompt) = status.prompt {
        prompt.active = false;
    }
    if let Some(mut context) = status.turn_context {
        context.current_player = turn.color;
        context.move_number = turn.move_number;
        context.phase = crate::game::resources::TurnPhase::WaitingForInput;
    }
    epoch.bump();
    premove.clear();
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    if let (Some(meshes), Some(mut materials)) = (meshes, materials) {
        spawn_pieces_from_fen(
            &mut commands,
            &meshes,
            &mut materials,
            &fen,
            Vec3::ZERO,
            &sprites,
        );
    } else {
        // Headless clients still own the same authoritative ECS board.
        for (row, line) in fen
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('/')
            .enumerate()
        {
            let mut file = 0;
            for c in line.chars() {
                if let Some(n) = c.to_digit(10) {
                    file += n as u8;
                    continue;
                }
                let color = if c.is_ascii_uppercase() {
                    crate::rendering::pieces::PieceColor::White
                } else {
                    crate::rendering::pieces::PieceColor::Black
                };
                let kind = match c.to_ascii_lowercase() {
                    'p' => Some(crate::rendering::pieces::PieceType::Pawn),
                    'k' => Some(crate::rendering::pieces::PieceType::King),
                    _ => crate::rendering::pieces::PieceType::from_char(c),
                };
                if let Some(kind) = kind {
                    let square = (file, 7 - row as u8);
                    commands.spawn((
                        Piece::new(color, kind, square.0, square.1),
                        HasMoved::default(),
                        Transform::from_translation(board_to_world(square_to_board(square))),
                        crate::core::game_scene(),
                    ));
                }
                file += 1;
            }
        }
    }
    commands.queue(move |world: &mut World| {
        stamp_fen_history(world, &fen);
        VerifiedBoardPosition::record(world, fen);
    });
    if let Some(mut spawned) = spawned {
        spawned.spawned = true;
    }
}

/// FEN has no complete movement history, but it does specify the remaining
/// castling rights. Preserve those when the existing engine sync reads ECS.
pub(crate) fn stamp_fen_history(world: &mut World, fen: &str) {
    use crate::rendering::pieces::{PieceColor, PieceType};
    let rights = fen.split_whitespace().nth(2).unwrap_or("-");
    let mut pieces = world.query::<(&Piece, &mut HasMoved)>();
    for (p, mut history) in pieces.iter_mut(world) {
        history.moved = match (p.piece_type, p.color, p.x, p.y) {
            (PieceType::King, PieceColor::White, 4, 0) => !rights.contains(['K', 'Q']),
            (PieceType::King, PieceColor::Black, 4, 7) => !rights.contains(['k', 'q']),
            (PieceType::Rook, PieceColor::White, 0, 0) => !rights.contains('Q'),
            (PieceType::Rook, PieceColor::White, 7, 0) => !rights.contains('K'),
            (PieceType::Rook, PieceColor::Black, 0, 7) => !rights.contains('q'),
            (PieceType::Rook, PieceColor::Black, 7, 7) => !rights.contains('k'),
            (PieceType::Pawn, PieceColor::White, _, 1)
            | (PieceType::Pawn, PieceColor::Black, _, 6) => false,
            _ => true,
        };
        history.move_count = u32::from(history.moved);
    }
}

/// Peer snapshots must prove a legal history through our accepted baseline,
/// with exact FENs and a matching head. They cannot choose a conflicting branch.
pub(crate) fn verify_snapshot_history(
    baseline: &str,
    fen: &str,
    moves: &[braid_chess::MovePayload],
    head: &str,
) -> bool {
    use crate::multiplayer::systems::fen_ply;
    let Some(anchor_ply) = fen_ply(baseline) else {
        return false;
    };
    if moves.len() < anchor_ply as usize || moves.len() > u16::MAX as usize {
        return false;
    }
    let mut position = xfchess_rules::Game::default();
    if anchor_ply == 0 && position.fen() != baseline {
        return false;
    }
    for (index, payload) in moves.iter().enumerate() {
        if position.play_uci(&payload.uci).is_err() || position.fen() != payload.fen_after {
            return false;
        }
        if index as u64 + 1 == anchor_ply && payload.fen_after != baseline {
            return false;
        }
    }
    let expected_head = moves
        .last()
        .map(|payload| braid_chess::version_hash(&payload.fen_after, payload.move_number))
        .unwrap_or_else(|| "0".into());
    position.fen() == fen && expected_head == head
}
